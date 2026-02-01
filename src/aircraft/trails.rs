use bevy::prelude::*;
use std::collections::VecDeque;
use std::time::Instant;

/// A single point in the trail history
#[derive(Clone, Debug)]
pub struct TrailPoint {
    pub lat: f64,
    pub lon: f64,
    pub altitude: Option<i32>,
    pub timestamp: Instant,
}

/// Component storing trail history for an aircraft
#[derive(Component, Default)]
pub struct TrailHistory {
    pub points: VecDeque<TrailPoint>,
}

/// Resource for trail configuration
#[derive(Resource)]
pub struct TrailConfig {
    pub enabled: bool,
    pub max_age_seconds: u64,
    pub solid_duration_seconds: u64,
    pub fade_duration_seconds: u64,
}

impl Default for TrailConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_age_seconds: 300,
            solid_duration_seconds: 225,
            fade_duration_seconds: 75,
        }
    }
}

impl TrailHistory {
    /// Add a new point to the trail
    pub fn add_point(&mut self, lat: f64, lon: f64, altitude: Option<i32>) {
        self.points.push_back(TrailPoint {
            lat,
            lon,
            altitude,
            timestamp: Instant::now(),
        });
    }

    /// Remove points older than max_age
    pub fn prune(&mut self, max_age_seconds: u64) {
        let cutoff = Instant::now() - std::time::Duration::from_secs(max_age_seconds);
        while let Some(front) = self.points.front() {
            if front.timestamp < cutoff {
                self.points.pop_front();
            } else {
                break;
            }
        }
    }
}

/// Get color for altitude (cyan at low, purple at high)
pub fn altitude_color(altitude: Option<i32>) -> Color {
    let alt = altitude.unwrap_or(0).max(0) as f32;

    // Altitude ranges: 0-10k cyan, 10k-20k green, 20k-30k yellow, 30k-40k orange, 40k+ purple
    let t = (alt / 40000.0).clamp(0.0, 1.0);

    if t < 0.25 {
        // Cyan to green
        let s = t / 0.25;
        Color::srgb(0.0, 1.0 - s * 0.5, 1.0 - s)
    } else if t < 0.5 {
        // Green to yellow
        let s = (t - 0.25) / 0.25;
        Color::srgb(s, 0.5 + s * 0.5, 0.0)
    } else if t < 0.75 {
        // Yellow to orange
        let s = (t - 0.5) / 0.25;
        Color::srgb(1.0, 1.0 - s * 0.4, 0.0)
    } else {
        // Orange to purple
        let s = (t - 0.75) / 0.25;
        Color::srgb(1.0 - s * 0.2, 0.6 - s * 0.6, s)
    }
}

/// Calculate opacity based on age
pub fn age_opacity(timestamp: Instant, solid_secs: u64, fade_secs: u64) -> f32 {
    let age = timestamp.elapsed().as_secs_f32();
    let solid = solid_secs as f32;
    let fade = fade_secs as f32;

    if age < solid {
        1.0
    } else if age < solid + fade {
        1.0 - (age - solid) / fade
    } else {
        0.0
    }
}

/// Resource to track when we last recorded trail points
#[derive(Resource)]
pub struct TrailRecordTimer {
    pub last_record: Instant,
    pub interval_secs: f32,
}

impl Default for TrailRecordTimer {
    fn default() -> Self {
        Self {
            last_record: Instant::now(),
            interval_secs: 2.0, // Record position every 2 seconds
        }
    }
}

/// System to record aircraft positions into trail history
pub fn record_trail_points(
    mut timer: ResMut<TrailRecordTimer>,
    config: Res<TrailConfig>,
    mut query: Query<(&crate::Aircraft, &mut TrailHistory)>,
) {
    if !config.enabled {
        return;
    }

    let now = Instant::now();
    if now.duration_since(timer.last_record).as_secs_f32() < timer.interval_secs {
        return;
    }
    timer.last_record = now;

    for (aircraft, mut trail) in query.iter_mut() {
        trail.add_point(aircraft.latitude, aircraft.longitude, aircraft.altitude);
    }
}
