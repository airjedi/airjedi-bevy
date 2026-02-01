use bevy::prelude::*;
use std::collections::HashMap;

use crate::data::{cache_path, is_cache_fresh, download_file_blocking, DataFile};
use super::types::{Airport, Runway, Navaid};

/// Resource containing all aviation data
#[derive(Resource, Default)]
pub struct AviationData {
    pub airports: Vec<Airport>,
    pub runways: Vec<Runway>,
    pub navaids: Vec<Navaid>,
    /// Runways indexed by airport_ref for fast lookup
    pub runways_by_airport: HashMap<i64, Vec<usize>>,
    pub loading_state: LoadingState,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum LoadingState {
    #[default]
    NotStarted,
    Downloading,
    Parsing,
    Ready,
    Failed,
}

impl AviationData {
    /// Build runway index after loading
    pub fn build_runway_index(&mut self) {
        self.runways_by_airport.clear();
        for (idx, runway) in self.runways.iter().enumerate() {
            self.runways_by_airport
                .entry(runway.airport_ref)
                .or_default()
                .push(idx);
        }
    }

    /// Get runways for an airport
    pub fn get_runways_for_airport(&self, airport_id: i64) -> Vec<&Runway> {
        self.runways_by_airport
            .get(&airport_id)
            .map(|indices| indices.iter().map(|&i| &self.runways[i]).collect())
            .unwrap_or_default()
    }
}

/// Load airports from cached CSV
fn load_airports() -> Result<Vec<Airport>, String> {
    let path = cache_path(DataFile::Airports.filename());
    let mut rdr = csv::Reader::from_path(&path)
        .map_err(|e| format!("Failed to open airports.csv: {}", e))?;

    let mut airports = Vec::new();
    for result in rdr.deserialize() {
        match result {
            Ok(airport) => airports.push(airport),
            Err(e) => {
                // Log but continue - some rows may have parsing issues
                warn!("Skipping airport row: {}", e);
            }
        }
    }
    info!("Loaded {} airports", airports.len());
    Ok(airports)
}

/// Load runways from cached CSV
fn load_runways() -> Result<Vec<Runway>, String> {
    let path = cache_path(DataFile::Runways.filename());
    let mut rdr = csv::Reader::from_path(&path)
        .map_err(|e| format!("Failed to open runways.csv: {}", e))?;

    let mut runways = Vec::new();
    for result in rdr.deserialize() {
        match result {
            Ok(runway) => runways.push(runway),
            Err(e) => {
                warn!("Skipping runway row: {}", e);
            }
        }
    }
    info!("Loaded {} runways", runways.len());
    Ok(runways)
}

/// Load navaids from cached CSV
fn load_navaids() -> Result<Vec<Navaid>, String> {
    let path = cache_path(DataFile::Navaids.filename());
    let mut rdr = csv::Reader::from_path(&path)
        .map_err(|e| format!("Failed to open navaids.csv: {}", e))?;

    let mut navaids = Vec::new();
    for result in rdr.deserialize() {
        match result {
            Ok(navaid) => navaids.push(navaid),
            Err(e) => {
                warn!("Skipping navaid row: {}", e);
            }
        }
    }
    info!("Loaded {} navaids", navaids.len());
    Ok(navaids)
}

/// System to initialize aviation data loading
pub fn start_aviation_data_loading(mut aviation_data: ResMut<AviationData>) {
    if aviation_data.loading_state != LoadingState::NotStarted {
        return;
    }

    aviation_data.loading_state = LoadingState::Downloading;

    // Check cache freshness and download if needed
    let files = [DataFile::Airports, DataFile::Runways, DataFile::Navaids];

    for file in &files {
        if !is_cache_fresh(file.filename()) {
            if let Err(e) = download_file_blocking(file) {
                error!("Failed to download {}: {}", file.filename(), e);
                aviation_data.loading_state = LoadingState::Failed;
                return;
            }
        }
    }

    aviation_data.loading_state = LoadingState::Parsing;

    // Load data from cache
    match (load_airports(), load_runways(), load_navaids()) {
        (Ok(airports), Ok(runways), Ok(navaids)) => {
            aviation_data.airports = airports;
            aviation_data.runways = runways;
            aviation_data.navaids = navaids;
            aviation_data.build_runway_index();
            aviation_data.loading_state = LoadingState::Ready;
            info!("Aviation data ready");
        }
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            error!("Failed to load aviation data: {}", e);
            aviation_data.loading_state = LoadingState::Failed;
        }
    }
}
