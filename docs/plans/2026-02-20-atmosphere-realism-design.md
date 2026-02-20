# Atmosphere Realism Design

Date: 2026-02-20

## Goal

Improve flight simulator realism of AirJedi's 3D view by upgrading solar positioning accuracy, atmosphere and fog rendering, night sky quality, moonlight, and adding time-of-day awareness to 2D mode.

## Constraints

- Visuals first; weather API integration deferred to a later effort
- No shadow maps (performance over visual fidelity for a tracker app)
- No visible moon disc; moonlight only via secondary DirectionalLight
- Wall-clock time by default with a 24-hour slider override for preview
- Improved procedural stars (no real star catalog)
- 3D mode gets full atmosphere; 2D mode gets subtle time-of-day tinting

## Current State

`view3d/sky.rs` (453 lines) provides:

- Simplified solar ephemeris (~1 degree accuracy, hand-rolled J2000 algorithm)
- 800 procedural stars on a 2048x2048 texture, binary visible/hidden at elevation 0
- `DistanceFog` with `FogFalloff::Exponential`, sun-dependent color transitions
- `DirectionalLight` at 5000 lux (flat, no pre-scattering constant)
- `Atmosphere::earthlike()` with `AtmosphereSettings` on Camera3d
- `GlobalAmbientLight` scaling 80-300 based on elevation
- Dark ground plane mesh

## Phases

### Phase 1: Accurate Solar Positioning + Time Controls

Replace `compute_sun_position()` with the `solar-positioning` crate (SPA algorithm, 0.0003 degree accuracy, polar day/night support). Add a `TimeState` resource holding either wall-clock UTC or a manual override. Add a 24-hour time slider to the View3D egui panel. Switch DirectionalLight illuminance to ~128,000 lux (raw sunlight pre-scattering) scaled by elevation. Smooth the ambient light twilight curve.

Files: `Cargo.toml`, `sky.rs`, View3D panel code.

Dependencies: `solar-positioning`, `chrono` (already present).

### Phase 2: Atmosphere and Fog Tuning

Switch fog from `FogFalloff::Exponential` to `FogFalloff::from_visibility_colors()` with extinction and inscattering colors that shift with sun elevation. Add `AtmosphereEnvironmentMapLight` to Camera3d for indirect sky illumination. Improve fog color transitions with civil/nautical/astronomical twilight zones and more saturated sunrise/sunset colors. Tweak ground plane material roughness.

Files: `sky.rs`, `main.rs` (camera setup).

Depends on: Phase 1 (uses improved SunState and TimeState).

### Phase 3: Enhanced Night Sky

Increase star count from 800 to ~3000 with realistic magnitude distribution (many dim, few bright). Add a Milky Way band as a gaussian concentration of extra-dim stars across a diagonal belt. Replace binary star visibility with gradual alpha fade during twilight. Add subtle twinkling via per-frame sine-based alpha oscillation on brightest stars. Increase texture resolution from 2048 to 4096.

Files: `sky.rs` (star generation, visibility system).

Independent of other phases.

### Phase 4: Moonlight

Add simplified moon position algorithm (J2000-based). Spawn a second DirectionalLight with `MoonLight` marker, cool blue-white color at 0.1-0.3 lux. Scale illuminance by approximate lunar phase (synodic month calculation). Moon elevation affects night ambient brightness. Respects `TimeState` override.

Files: `sky.rs` (or new `moon.rs` if substantial), new `MoonState` resource.

Depends on: Phase 1 (uses TimeState).

### Phase 5: 2D Mode Time-of-Day Tinting

Apply a subtle full-screen color overlay in 2D mode above map tiles (z=5) but below aircraft (z=10). Golden hour: warm amber at ~10% opacity. Night: cool blue-black at ~30% opacity. Midday: fully transparent. Uses same `SunState` and `TimeState` as 3D mode. Tint strength configurable or disableable in settings.

Files: `sky.rs` (new 2D tint system), settings panel.

Independent of other phases (only needs existing SunState).

## Parallelization

| Branch | Phase | Can Run With |
|---|---|---|
| `feat/solar-accuracy` | 1: Solar + Time | 3, 5 |
| `feat/atmosphere-tuning` | 2: Atmosphere/Fog | 3 (after Phase 1 merges) |
| `feat/night-sky` | 3: Star Field | 1, 4, 5 |
| `feat/moonlight` | 4: Moonlight | 3 (after Phase 1 merges) |
| `feat/2d-tinting` | 5: 2D Tinting | 1, 3 |

Phases 1, 3, and 5 are fully independent and can run simultaneously. Phases 2 and 4 depend on Phase 1 for TimeState and improved SunState.

## Key Technical References

- Bevy 0.18 Atmosphere: `Atmosphere::earthlike()`, `AtmosphereSettings`, `AtmosphereEnvironmentMapLight`, `ScatteringMedium`
- Bevy DistanceFog: `FogFalloff::from_visibility_colors()` with extinction/inscattering colors
- `solar-positioning` crate: SPA algorithm, `spa::solar_position()`, `SolarPosition::azimuth()`, `SolarPosition::elevation_angle()`
- DirectionalLight: `lux::RAW_SUNLIGHT` (~128,000 lux), transform forward axis = light direction
- Fog color reference: blue-gray midday, amber golden hour, dark twilight zones at -6/-12/-18 degrees
