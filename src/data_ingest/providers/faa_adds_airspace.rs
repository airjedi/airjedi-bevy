use chrono::Utc;
use serde_json::Value;

use crate::data_ingest::canonical::{AirspaceInfo, CanonicalRecord};
use crate::data_ingest::pipeline::{PipelineData, PipelineError, PipelinePhase, PipelineStage};
use crate::data_ingest::provider::{
    DataProvider, FetchContext, ProviderCategory, ProviderError, ProviderMeta, RawFetchResult,
};

const FAA_ADDS_AIRSPACE_URL: &str = "https://services6.arcgis.com/ssFJjBXIUyZDrSYZ/arcgis/rest/services/Class_Airspace/FeatureServer/0/query";

/// Data provider that fetches Class B/C and Special Use Airspace from
/// the FAA ADDS ArcGIS FeatureServer.
pub struct FaaClassAirspaceProvider;

impl DataProvider for FaaClassAirspaceProvider {
    fn name(&self) -> &str {
        "faa_adds_airspace"
    }

    fn schedule(&self) -> &str {
        "0 0 4 * * *"
    }

    fn metadata(&self) -> ProviderMeta {
        ProviderMeta {
            display_name: "FAA Airspace",
            category: ProviderCategory::Navigation,
            description: "Class B/C and special use airspace from FAA ADDS",
            config_key: "faa_airspace",
        }
    }

    fn fetch(&self, _ctx: &FetchContext) -> Result<RawFetchResult, ProviderError> {
        // Bulk fetch — no bounding box, get all B/C + SUA
        let url = format!(
            "{}?where={}&outFields=*&f=geojson",
            FAA_ADDS_AIRSPACE_URL,
            "CLASS+IN+('B','C')+OR+TYPE+IN+('R','MOA','W','A')"
        );

        let response = reqwest::blocking::get(&url)
            .map_err(|e| ProviderError::Network(format!("FAA ADDS airspace fetch failed: {}", e)))?;

        let bytes = response
            .bytes()
            .map_err(|e| ProviderError::Network(format!("Failed to read response: {}", e)))?;

        Ok(RawFetchResult {
            data: bytes.to_vec(),
            content_type: Some("application/json".to_string()),
            source: url,
        })
    }

    fn pipeline_stages(&self) -> Vec<Box<dyn PipelineStage>> {
        vec![Box::new(FaaAirspaceParseStage)]
    }
}

struct FaaAirspaceParseStage;

impl PipelineStage for FaaAirspaceParseStage {
    fn name(&self) -> &str {
        "faa_adds_airspace_parse"
    }

    fn phase(&self) -> PipelinePhase {
        PipelinePhase::Parse
    }

    fn execute(&self, data: &mut PipelineData) -> Result<(), PipelineError> {
        let bytes = data
            .raw_bytes
            .as_ref()
            .ok_or_else(|| PipelineError::StageError { stage: "faa_adds_airspace_parse".into(), message: "No raw data to parse".into() })?;

        let geojson: Value = serde_json::from_slice(bytes)
            .map_err(|e| PipelineError::StageError { stage: "faa_adds_airspace_parse".into(), message: format!("Invalid JSON: {}", e) })?;

        let features = geojson["features"]
            .as_array()
            .ok_or_else(|| PipelineError::StageError { stage: "faa_adds_airspace_parse".into(), message: "No 'features' array in GeoJSON".into() })?;

        let now = Utc::now();
        let mut records = Vec::new();

        for feature in features {
            let props = &feature["properties"];

            let ident = props["IDENT"].as_str().unwrap_or("UNKNOWN");
            let name = props["NAME"].as_str().unwrap_or(ident);
            let class = props["CLASS"].as_str().unwrap_or("");
            let type_code = props["TYPE"].as_str().unwrap_or("");

            // Altitude values are in hundreds of feet
            let upper_val = props["UPPER_VAL"].as_i64().map(|v| (v * 100) as i32);
            let lower_val = props["LOWER_VAL"].as_i64().map(|v| (v * 100) as i32);
            let upper_code = props["UPPER_CODE"].as_str().map(String::from);
            let lower_code = props["LOWER_CODE"].as_str().map(String::from);

            // Determine airspace class string for canonical record
            let airspace_class = match class {
                "B" => "ClassB",
                "C" => "ClassC",
                _ => match type_code {
                    "R" => "Restricted",
                    "MOA" => "MOA",
                    "W" => "Warning",
                    "A" => "Alert",
                    _ => continue, // skip unknown types
                },
            };

            // Extract polygon coordinates from GeoJSON geometry
            let polygon = extract_polygon_coords(&feature["geometry"]);
            if polygon.is_empty() {
                continue;
            }

            records.push(CanonicalRecord::Airspace(AirspaceInfo {
                name: format!("{} {}", name, ident),
                airspace_class: airspace_class.to_string(),
                airspace_type: if class.is_empty() {
                    type_code.to_string()
                } else {
                    format!("Class {}", class)
                },
                lower_limit_ft: lower_val,
                upper_limit_ft: upper_val,
                lower_altitude_ref: lower_code,
                upper_altitude_ref: upper_code,
                polygon,
                fetched_at: now,
            }));
        }

        data.records = records;
        Ok(())
    }
}

/// Extract (lat, lon) pairs from a GeoJSON geometry (Polygon or MultiPolygon).
fn extract_polygon_coords(geometry: &Value) -> Vec<(f64, f64)> {
    let geo_type = geometry["type"].as_str().unwrap_or("");
    let coords = &geometry["coordinates"];

    match geo_type {
        "Polygon" => {
            // coords[0] = outer ring = [[lon, lat], ...]
            extract_ring(&coords[0])
        }
        "MultiPolygon" => {
            // coords[0][0] = first polygon's outer ring
            extract_ring(&coords[0][0])
        }
        _ => Vec::new(),
    }
}

fn extract_ring(ring: &Value) -> Vec<(f64, f64)> {
    ring.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|coord| {
                    let lon = coord[0].as_f64()?;
                    let lat = coord[1].as_f64()?;
                    Some((lat, lon))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_faa_airspace_geojson() {
        let geojson = r#"{
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "properties": {
                    "IDENT": "KICT",
                    "NAME": "WICHITA",
                    "CLASS": "C",
                    "UPPER_VAL": 53,
                    "UPPER_CODE": "MSL",
                    "LOWER_VAL": 0,
                    "LOWER_CODE": "SFC",
                    "TYPE": ""
                },
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [[
                        [-97.43, 37.65],
                        [-97.20, 37.65],
                        [-97.20, 37.72],
                        [-97.43, 37.72],
                        [-97.43, 37.65]
                    ]]
                }
            }]
        }"#;

        let mut data = PipelineData {
            raw_bytes: Some(geojson.as_bytes().to_vec()),
            records: Vec::new(),
            metadata: std::collections::HashMap::new(),
        };

        let stage = FaaAirspaceParseStage;
        stage.execute(&mut data).unwrap();

        assert_eq!(data.records.len(), 1);
        if let CanonicalRecord::Airspace(ref info) = data.records[0] {
            assert!(info.name.contains("KICT"));
            assert_eq!(info.airspace_class, "ClassC");
            assert_eq!(info.upper_limit_ft, Some(5300));
            assert_eq!(info.lower_limit_ft, Some(0));
            assert_eq!(info.upper_altitude_ref.as_deref(), Some("MSL"));
            assert_eq!(info.lower_altitude_ref.as_deref(), Some("SFC"));
            assert_eq!(info.polygon.len(), 5);
        } else {
            panic!("Expected Airspace record");
        }
    }

    #[test]
    fn test_parse_restricted_airspace() {
        let geojson = r#"{
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "properties": {
                    "IDENT": "R-2508",
                    "NAME": "CHINA LAKE",
                    "CLASS": "",
                    "UPPER_VAL": 999,
                    "UPPER_CODE": "MSL",
                    "LOWER_VAL": 0,
                    "LOWER_CODE": "SFC",
                    "TYPE": "R"
                },
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [[
                        [-117.8, 35.6],
                        [-117.5, 35.6],
                        [-117.5, 35.9],
                        [-117.8, 35.9],
                        [-117.8, 35.6]
                    ]]
                }
            }]
        }"#;

        let mut data = PipelineData {
            raw_bytes: Some(geojson.as_bytes().to_vec()),
            records: Vec::new(),
            metadata: std::collections::HashMap::new(),
        };

        let stage = FaaAirspaceParseStage;
        stage.execute(&mut data).unwrap();

        assert_eq!(data.records.len(), 1);
        if let CanonicalRecord::Airspace(ref info) = data.records[0] {
            assert_eq!(info.airspace_class, "Restricted");
            assert_eq!(info.upper_limit_ft, Some(99900));
        } else {
            panic!("Expected Airspace record");
        }
    }

    #[test]
    fn test_extract_multipolygon() {
        let geo = serde_json::json!({
            "type": "MultiPolygon",
            "coordinates": [[[
                [-97.43, 37.65],
                [-97.20, 37.65],
                [-97.20, 37.72],
                [-97.43, 37.72],
                [-97.43, 37.65]
            ]]]
        });

        let coords = extract_polygon_coords(&geo);
        assert_eq!(coords.len(), 5);
        assert!((coords[0].0 - 37.65).abs() < 0.001); // lat
        assert!((coords[0].1 - (-97.43)).abs() < 0.001); // lon
    }

    #[test]
    fn test_metadata() {
        let provider = FaaClassAirspaceProvider;
        let meta = provider.metadata();
        assert_eq!(meta.config_key, "faa_airspace");
        assert_eq!(meta.category, ProviderCategory::Navigation);
    }
}
