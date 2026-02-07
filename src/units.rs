//! Newtype wrappers for aviation units.
//!
//! These prevent accidental mixing of feet/meters, knots/km-h, etc.
//! Conversion methods are provided for common transformations.

use crate::geo::{FEET_TO_METERS, NM_TO_KM};

/// Altitude or vertical distance in feet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Feet(pub i32);

impl Feet {
    /// Convert to meters.
    pub fn to_meters(self) -> f64 {
        self.0 as f64 * FEET_TO_METERS
    }

    /// Convert to flight level (hundreds of feet).
    pub fn to_flight_level(self) -> i32 {
        self.0 / 100
    }
}

impl std::fmt::Display for Feet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ft", self.0)
    }
}

/// Speed in knots (nautical miles per hour).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Knots(pub f64);

impl Knots {
    /// Convert to km/h.
    pub fn to_kmh(self) -> f64 {
        self.0 * NM_TO_KM
    }

    /// Convert to m/s.
    pub fn to_ms(self) -> f64 {
        self.0 * NM_TO_KM * 1000.0 / 3600.0
    }
}

impl std::fmt::Display for Knots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} kts", self.0 as i32)
    }
}

/// Bearing or heading in degrees (0-360, clockwise from north).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Degrees(pub f32);

impl Degrees {
    /// Convert to radians.
    pub fn to_radians(self) -> f32 {
        self.0.to_radians()
    }

    /// Normalize to 0..360 range.
    pub fn normalized(self) -> Self {
        Self(((self.0 % 360.0) + 360.0) % 360.0)
    }
}

impl std::fmt::Display for Degrees {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.0}\u{00b0}", self.0) // degree sign
    }
}

/// Distance in nautical miles.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct NauticalMiles(pub f64);

impl NauticalMiles {
    /// Convert to kilometers.
    pub fn to_km(self) -> f64 {
        self.0 * NM_TO_KM
    }

    /// Convert to meters.
    pub fn to_meters(self) -> f64 {
        self.0 * NM_TO_KM * 1000.0
    }
}

impl std::fmt::Display for NauticalMiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1} NM", self.0)
    }
}
