use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// All canonical record types produced by data ingest providers.
/// Each variant wraps a domain-specific struct with normalized fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanonicalRecord {
    Metar(MetarReport),
    Taf(TafReport),
    Sigmet(SigmetReport),
    Airmet(AirmetReport),
    Pirep(PirepReport),
    Airport(AirportInfo),
    Runway(RunwayInfo),
    Navaid(NavaidInfo),
    Airway(AirwayInfo),
    Airspace(AirspaceInfo),
    Frequency(FrequencyInfo),
    Notam(NotamInfo),
    Tfr(TfrInfo),
}

impl CanonicalRecord {
    /// Returns the kind label for logging and metrics.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Metar(_) => "metar",
            Self::Taf(_) => "taf",
            Self::Sigmet(_) => "sigmet",
            Self::Airmet(_) => "airmet",
            Self::Pirep(_) => "pirep",
            Self::Airport(_) => "airport",
            Self::Runway(_) => "runway",
            Self::Navaid(_) => "navaid",
            Self::Airway(_) => "airway",
            Self::Airspace(_) => "airspace",
            Self::Frequency(_) => "frequency",
            Self::Notam(_) => "notam",
            Self::Tfr(_) => "tfr",
        }
    }
}

// ---------------------------------------------------------------------------
// Weather
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetarReport {
    pub icao: String,
    pub raw_text: String,
    pub observation_time: DateTime<Utc>,
    pub wind_direction_deg: Option<u16>,
    pub wind_speed_kt: Option<u16>,
    pub wind_gust_kt: Option<u16>,
    pub visibility_sm: Option<f32>,
    pub ceiling_ft: Option<i32>,
    pub temperature_c: Option<f32>,
    pub dewpoint_c: Option<f32>,
    pub altimeter_inhg: Option<f32>,
    pub flight_category: String,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TafReport {
    pub icao: String,
    pub raw_text: String,
    pub issue_time: DateTime<Utc>,
    pub valid_from: DateTime<Utc>,
    pub valid_to: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmetReport {
    pub id: String,
    pub region: String,
    pub hazard: String,
    pub raw_text: String,
    pub valid_from: DateTime<Utc>,
    pub valid_to: DateTime<Utc>,
    pub min_altitude_ft: Option<i32>,
    pub max_altitude_ft: Option<i32>,
    pub polygon: Vec<(f64, f64)>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirmetReport {
    pub id: String,
    pub region: String,
    pub hazard: String,
    pub raw_text: String,
    pub valid_from: DateTime<Utc>,
    pub valid_to: DateTime<Utc>,
    pub polygon: Vec<(f64, f64)>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PirepReport {
    pub raw_text: String,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_ft: i32,
    pub observation_time: DateTime<Utc>,
    pub aircraft_type: Option<String>,
    pub report_type: String,
    pub fetched_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirportInfo {
    pub ident: String,
    pub name: String,
    pub airport_type: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_ft: Option<i32>,
    pub iso_country: String,
    pub iso_region: String,
    pub municipality: Option<String>,
    pub scheduled_service: bool,
    pub iata_code: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunwayInfo {
    pub airport_ident: String,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub surface: Option<String>,
    pub lighted: bool,
    pub closed: bool,
    pub le_ident: String,
    pub le_latitude: Option<f64>,
    pub le_longitude: Option<f64>,
    pub le_heading_deg: Option<f32>,
    pub he_ident: String,
    pub he_latitude: Option<f64>,
    pub he_longitude: Option<f64>,
    pub he_heading_deg: Option<f32>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavaidInfo {
    pub ident: String,
    pub name: String,
    pub navaid_type: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_ft: Option<i32>,
    pub frequency_khz: Option<u32>,
    pub associated_airport: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirwayInfo {
    pub designator: String,
    pub airway_type: String,
    pub sequence: u32,
    pub fix_ident: String,
    pub fix_latitude: f64,
    pub fix_longitude: f64,
    pub min_altitude_ft: Option<i32>,
    pub max_altitude_ft: Option<i32>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirspaceInfo {
    pub name: String,
    pub airspace_class: String,
    pub airspace_type: String,
    pub lower_limit_ft: Option<i32>,
    pub upper_limit_ft: Option<i32>,
    pub polygon: Vec<(f64, f64)>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyInfo {
    pub airport_ident: String,
    pub frequency_type: String,
    pub description: String,
    pub frequency_mhz: f64,
    pub fetched_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Notices
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotamInfo {
    pub id: String,
    pub location: String,
    pub raw_text: String,
    pub classification: String,
    pub effective_start: DateTime<Utc>,
    pub effective_end: Option<DateTime<Utc>>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_nm: Option<f64>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfrInfo {
    pub notam_id: String,
    pub name: String,
    pub tfr_type: String,
    pub effective_start: DateTime<Utc>,
    pub effective_end: Option<DateTime<Utc>>,
    pub lower_altitude_ft: Option<i32>,
    pub upper_altitude_ft: Option<i32>,
    pub polygon: Vec<(f64, f64)>,
    pub fetched_at: DateTime<Utc>,
}
