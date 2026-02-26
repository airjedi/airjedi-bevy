# Aircraft Model Registry - Type-Specific 3D Models

**Date:** 2026-02-25

## Goal

Load aircraft-type-specific 3D models so that B737s (and eventually other types) render with their actual aircraft model instead of the generic `airplane.glb`.

## Approach

Add the `bevy_obj` crate (v0.18) to load Wavefront OBJ files natively. Replace the single `AircraftModel` resource with an `AircraftModelRegistry` that maps type code prefixes to scene handles. When spawning aircraft, look up the type code to select the right model; fall back to the generic model for unknown types.

## Asset Structure

```
assets/
  airplane.glb                  (existing generic model)
  models/
    b737/
      78349.obj                 (B737 model)
      scene1.mtl                (materials)
```

Copy `~/Downloads/78349.obj` and `~/Downloads/78349.mtl` into `assets/models/b737/`.

## Changes

### 1. Cargo.toml
Add `bevy_obj = "0.18"` dependency.

### 2. src/main.rs
Add `ObjPlugin` to app plugins.

### 3. src/adsb/sync.rs
- Replace `AircraftModel` (single handle) with `AircraftModelRegistry`:
  ```rust
  #[derive(Resource)]
  pub struct AircraftModelRegistry {
      pub default_model: Handle<Scene>,
      pub type_models: HashMap<String, Handle<Scene>>,
  }
  ```
- `setup_aircraft_model` becomes `setup_aircraft_models`: loads the generic model and the B737 OBJ, registers "B73" prefix -> B737 handle.
- Add a method `get_model_for_type(&self, type_code: Option<&str>) -> Handle<Scene>` that checks type_code prefixes against the registry and returns the matching handle or the default.

### 4. src/adsb/sync.rs - sync_aircraft_from_adsb
- When spawning a new aircraft, look up `AircraftTypeInfo` from the `AircraftTypeDatabase` to get the type_code.
- Pass the type_code to the registry to get the right scene handle.
- Aircraft without type info yet get the generic model.

### 5. Model Update System (optional, deferred)
Aircraft that spawn before their type info loads will use the generic model. A future enhancement could swap models once type info arrives, but this is out of scope for the initial implementation.

## Type Code Matching

The OpenSky database uses ICAO type designators. B737 variants use codes like:
- B731, B732, B733, B734, B735, B736, B737, B738, B739, B37M, B38M, B39M

Matching on prefix "B73" covers the classic variants. "B37M", "B38M", "B39M" (MAX variants) need explicit entries or a list-based lookup.

Registry will use a list of type codes rather than prefix matching:
```rust
const B737_TYPES: &[&str] = &["B731","B732","B733","B734","B735","B736","B737","B738","B739","B37M","B38M","B39M"];
```

## No Other Changes

- No changes to the Aircraft component, camera, view modes, or UI.
- The model renders in both 2D (as SceneRoot, hidden) and 3D views, same as today.
- Scale uses existing `AIRCRAFT_MODEL_SCALE` constant.
