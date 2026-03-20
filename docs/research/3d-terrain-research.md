# 3D Terrain Rendering Research — AirJedi-Bevy

> Research date: 2026-03-19
> Branch: `worktree-feature/3d-terrain-research`

## Executive Summary

This document consolidates research across three domains — **elevation data sources**, **terrain mesh generation techniques**, and **existing codebase integration** — to recommend the most efficient and maintainable approach for adding 3D terrain to AirJedi as an alternative to flat map tiles.

**Top-level recommendation:** Use **AWS Terrain Tiles (Terrarium PNG)** as the elevation data source, generate terrain meshes on CPU initially using **uniform grids (32×32 to 64×64)**, integrate at the `sync_tile_mesh_quads` insertion point in `tiles.rs`, and evolve toward **CDLOD with GPU vertex displacement** for production performance.

---

## Part 1: Elevation Data Sources

### Comparison Matrix

| Source | Resolution | Coverage | Cost | Auth | Tile Scheme | Rust Integration |
|--------|-----------|----------|------|------|-------------|-----------------|
| **AWS Terrain Tiles** | ~4.8m @ z15 | Global + bathymetry | **Free** | None | `{z}/{x}/{y}` slippy | Trivial (PNG decode) |
| Mapbox Terrain-RGB | ~5m @ z14 | Global | Usage-based | API key | `{z}/{x}/{y}` slippy | Trivial (PNG decode) |
| Cesium Quantized Mesh | Varies | Global | Free tier (MapTiler) | API key | TMS tiles | `quantized-mesh-decoder` crate |
| SRTM | 30m / 90m | 60°N–60°S | Free | None | 1°×1° bulk files | `srtm` / `srtm_reader` crates |
| USGS 3DEP | 1m–10m | US only | Free | None | Non-standard (COG) | `gdal` crate (heavy native dep) |
| OpenTopography | Sub-meter (LiDAR) | Regional | Free (50 req/day) | API key | Bounding-box API | `gdal` / `las` crates |

### Recommendation: AWS Terrain Tiles (Primary)

**Why this wins for AirJedi:**

1. **Same tile scheme** — Identical `{z}/{x}/{y}` Web Mercator coordinates as existing OSM tiles. Fetch elevation tiles alongside map tiles using the same infrastructure.
2. **Zero cost, zero auth** — No API key, no rate limits, no account. AWS Open Data Program covers hosting.
3. **Trivial decoding** — Terrarium PNG format: `height = R×256 + G + B/256 − 32768` meters. Decode with the `image` crate (already a dependency).
4. **Built-in LOD** — Lower zoom levels = coarser terrain automatically. Existing `altitude_to_zoom_level()` applies directly.
5. **Global coverage** — Composites 3DEP (US 3m/10m), SRTM (global 30m), GMTED (low zoom), ETOPO1 (ocean bathymetry).

**URL pattern:**
```
https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png
```

### Secondary: MapTiler Quantized Mesh (Future Optimization)

Pre-triangulated meshes with adaptive triangle density and edge-stitching metadata. The `quantized-mesh-decoder` crate outputs vertices + indices directly uploadable to Bevy. Consider this as a Phase 3 optimization when reducing CPU mesh generation overhead becomes important.

### Not Recommended

- **SRTM raw**: No LOD pyramid, 25MB per tile, not designed for streaming
- **USGS 3DEP**: US-only, non-standard tile access, heavy GDAL dependency
- **OpenTopography**: 50 requests/day rate limit, bounding-box API
- **Mapbox Terrain-RGB**: Functionally equivalent to AWS but requires API key and has usage costs

---

## Part 2: Terrain Mesh Generation

### Grid Tessellation Strategy

**Uniform grid is recommended** over adaptive tessellation (ROAM, progressive meshes). Reasons:
- Simple, predictable, GPU cache-coherent
- Aligns naturally with tile grid
- Flat area vertex waste is bounded and acceptable

**Per-tile grid resolution targets:**

| Grid Size | Vertices | Triangles | Use Case |
|-----------|----------|-----------|----------|
| 16×16 | 256 | 450 | LOD 3+ (distant/horizon tiles) |
| 32×32 | 1,024 | 1,922 | LOD 1-2 (mid-distance) |
| 64×64 | 4,096 | 7,938 | LOD 0 (near tiles) |

With 100–200 visible tiles, total vertex count stays in the **200K–800K range** — well within desktop GPU budgets.

### LOD Algorithm: CDLOD

**CDLOD (Continuous Distance-Dependent Level of Detail)** is the strongest match because:

1. **Quadtree-based** — Maps directly onto slippy tile quadtree (zoom level = depth)
2. **3D distance-based** — Critical for AirJedi where camera altitude ranges from 1,000 to 120,000 feet
3. **Shader-based vertex morphing** — Eliminates cracks between LOD levels without stitching geometry
4. **Existing zoom mapping** — `altitude_to_zoom_level()` already provides the LOD selection logic

**Crack prevention strategy:**
- Primary: CDLOD vertex morphing in vertex shader
- Fallback: Skirt geometry (vertical strips along tile edges extending downward)

### Phased Implementation

#### Phase 1: CPU Terrain Meshes (Start Here)
- Build `Mesh` on CPU using Bevy's `Mesh` API with `PrimitiveTopology::TriangleList`
- Set `ATTRIBUTE_POSITION`, `ATTRIBUTE_NORMAL`, `ATTRIBUTE_UV_0` + triangle indices
- 32×32 uniform grid per tile, heightmap displacement
- Normals computed via central finite differences
- Skirts for crack prevention
- Per-tile mesh handles (replaces shared `Plane3d`)
- Cost: ~0.5–2ms per 64×64 tile mesh, amortized across frames via async tasks

#### Phase 2: GPU Vertex Displacement
- Custom `Material` with WGSL vertex shader
- Revert to **single shared grid mesh** (like current shared `Plane3d`)
- Pass heightmap as GPU texture per tile
- Vertex shader samples heightmap and displaces Y coordinate
- Normal computation in vertex shader via central differences (3 heightmap samples)
- Eliminates CPU mesh generation and CPU→GPU transfer overhead

```wgsl
// Terrain vertex shader (Phase 2)
@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let uv = input.uv;
    let height = textureSampleLevel(heightmap, terrain_sampler, uv, 0.0).r;
    var world_pos = tile_transform * vec4(input.position, 1.0);
    world_pos.y += height * elevation_scale;

    // Normal from central differences
    let h_right = textureSampleLevel(heightmap, terrain_sampler, uv + vec2(texel_size, 0.0), 0.0).r;
    let h_up = textureSampleLevel(heightmap, terrain_sampler, uv + vec2(0.0, texel_size), 0.0).r;
    let normal = normalize(vec3(height - h_right, texel_size * elevation_scale, height - h_up));
    // ...
}
```

#### Phase 3: Full CDLOD + Instanced Rendering
- Quadtree node selection replaces flat tile grid
- Variable grid resolution per LOD level
- CDLOD vertex morphing uniforms in shader
- Single instanced draw call for all terrain tiles
- Optional: quantized mesh support via MapTiler

### GPU Tessellation Note

**wgpu does not support tessellation shaders** (open since 2019, still unimplemented). The vertex-displacement approach above is the standard alternative and is actually more portable and cache-friendly.

---

## Part 3: Integration with Existing Architecture

### Current Tile Pipeline

```
request_3d_tiles_continuous (300ms timer)
  → DownloadSlippyTilesMessage
  → SlippyTileDownloadedMessage
  → display_tiles_filtered (spawns 2D Sprite + MapTile + TileFadeState)
  → sync_tile_mesh_quads (creates flat Plane3d companion entity)
  → sync_tile_mesh_transforms (copies position Z-up → Y-up)
  → animate_tile_fades → cull_offscreen_tiles → cleanup
```

### Key Integration Point: `sync_tile_mesh_quads` (tiles.rs:919)

Currently this system:
1. Checks if tile lacks a `TileMeshQuad` component
2. **Clones the shared flat `Plane3d` mesh** ← THIS CHANGES
3. Creates `StandardMaterial` with tile texture as `emissive_texture`
4. Spawns companion entity on `RenderLayers::layer(TILES_3D)`

**With terrain:** Step 2 becomes "look up heightmap for this tile's (zoom, x, y), generate displaced mesh if available, else fall back to flat quad."

### What Changes vs What Stays

| Component | Verdict | Notes |
|-----------|---------|-------|
| `TileQuadMesh` (shared flat mesh) | **Replace** | Per-tile terrain mesh (Phase 1) or shared grid + shader (Phase 2) |
| `TileMeshQuad(Entity)` | **Keep** | Still links sprite to 3D companion |
| `TileQuad3d` marker | **Keep** | Still marks 3D entities |
| `sync_tile_mesh_quads` | **Extend** | Branch on `TerrainState.enabled` |
| `sync_tile_mesh_transforms` | **Keep** | Position sync unchanged |
| `TileFadeState` / fade animation | **Keep** | Orthogonal to mesh shape |
| `SpawnedTiles` dedup | **Keep** | Works regardless of mesh type |
| `cull_offscreen_tiles` | **Keep** | Distance-based culling is mesh-agnostic |
| `update_tile_elevation` | **Replace** | No longer single flat elevation; heights baked in mesh |
| `GroundPlane` | **Keep** | Fallback beyond tile coverage, Y = min visible elevation |
| `CoordinateConverter` | **Keep** | Lat/lon→pixel unchanged |
| `altitude_to_zoom_level` | **Keep** | Drives CDLOD node selection |
| `altitude_scale` (20×) | **Extend** | Apply same exaggeration to terrain vertex heights |
| Render layers | **Keep** | Terrain uses existing `TILES_3D` (layer 6) |

### Material Change: Emissive → Lit

**Critical architectural shift:** Current tile materials use `emissive_texture` (bypasses camera exposure). Terrain needs **lit materials** for visual depth from sun/moon directional light. This means:
- Use `base_color_texture` instead of `emissive_texture`
- Adjust exposure pipeline so terrain tiles aren't washed out
- Terrain benefits from the existing `DirectionalLight` sun/moon system in `sky.rs`

### Proposed Module Structure

```
src/
  terrain/
    mod.rs          — TerrainPlugin, TerrainState resource, feature flag
    heightmap.rs    — Elevation tile fetching, RGB decoding, HeightmapCache
    mesh.rs         — Terrain mesh generation (grid, displacement, normals, skirts)
    provider.rs     — Elevation data source abstraction (AWS, Mapbox, etc.)
    lod.rs          — LOD: mesh resolution based on distance/zoom
    stitch.rs       — Edge stitching between tiles at different zoom levels
```

### New ECS Resources

```rust
/// Whether terrain rendering is active (vs flat tiles)
struct TerrainState {
    enabled: bool,
    mesh_resolution: u32,        // 32 or 64
    elevation_scale: f32,        // matches altitude_scale
    provider: TerrainProvider,
}

/// Cache of decoded elevation grids
struct HeightmapCache(HashMap<(u8, u32, u32), Vec<f32>>);

/// Cache of generated terrain mesh handles
struct TerrainMeshCache(HashMap<(u8, u32, u32), Handle<Mesh>>);
```

### New ECS Components

```rust
/// Marker on terrain mesh entities
#[derive(Component)]
struct TerrainTile;

/// Tracks current LOD level for distance-based resolution
#[derive(Component)]
struct TerrainLod(u8);
```

### System Ordering

```
request_heightmap_tiles        (parallel to request_3d_tiles_continuous)
  → decode_heightmaps          (async, background thread)
  → generate_terrain_meshes    (replaces part of sync_tile_mesh_quads)
  → sync_tile_mesh_transforms  (existing, unchanged)
  → stitch_terrain_edges       (after generation)
```

---

## Part 4: Challenges & Mitigations

| Challenge | Impact | Mitigation |
|-----------|--------|------------|
| **Performance: 1000+ unique meshes** | CPU mesh generation bottleneck | Async generation on background threads; Phase 2 moves to GPU |
| **Multi-resolution seams** | Visible cracks between zoom levels | CDLOD morphing + skirt geometry fallback |
| **Heightmap/texture alignment** | Mismatched tile grids | Same `{z}/{x}/{y}` for both → perfect alignment |
| **Emissive → lit material** | Exposure/brightness pipeline change | Adjust `ev100` or use custom shader with exposure bypass |
| **Ground elevation model** | Single `ground_elevation_ft` becomes per-vertex | Query terrain height at aircraft position for clamping |
| **Memory pressure** | 2 textures per tile (satellite + heightmap) | Heightmap tiles are small (256×256 PNG ≈ 50KB); budget reduction to ~800 max tiles |
| **Tile rescaling on zoom change** | Flat scale doesn't work for terrain heights | Regenerate meshes at new zoom (or scale Y separately) |
| **2D ↔ 3D transition** | Flat→terrain animation | Interpolate displacement from 0→full over `TransitionState` duration |

---

## Part 5: Performance Budget (macOS Desktop)

| Metric | Budget | Notes |
|--------|--------|-------|
| Visible terrain tiles | 100–200 | Reduced from current 1500 flat tile budget |
| Vertices per tile | 1,024 (32²) – 4,096 (64²) | Distance-based LOD selection |
| Total vertices | 200K–800K | Well within Apple Silicon GPU capacity |
| Heightmap memory | 12–50 MB | 200 tiles × 256×256 × f32 |
| Mesh generation time | 0.5–2ms per tile | Amortized across frames via async |
| Draw calls | 100–200 (Phase 1) → 1 (Phase 3) | Instanced rendering in Phase 3 |
| Target framerate | 60 fps | Achievable on M1+ with Phase 1 |

---

## Part 6: Relevant Rust Crates

| Crate | Purpose | Phase |
|-------|---------|-------|
| `image` | Decode Terrarium PNG tiles | 1 (already a dependency) |
| `reqwest` | HTTP fetch elevation tiles | 1 (already a dependency) |
| `quantized-mesh-decoder` | Decode Cesium quantized mesh | 3 (optional) |
| `bevy_terrain` | Reference only (not compatible) | — |
| `srtm` / `srtm_reader` | SRTM .hgt parsing (offline preprocessing) | — |

No existing Bevy terrain crate is suitable for direct integration. AirJedi needs a **custom implementation** due to its unique requirements: real-world elevation data, dynamic slippy tile loading, satellite imagery textures, and the wide camera altitude range.

---

## References

- [AWS Terrain Tiles Registry](https://registry.opendata.aws/terrain-tiles/)
- [Terrarium Format Spec](https://github.com/tilezen/joerd/blob/master/docs/formats.md)
- [CDLOD Paper (Strugar)](https://aggrobird.com/files/cdlod_latest.pdf)
- [GPU Geometry Clipmaps (NVIDIA GPU Gems 2)](https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry)
- [wgpu Tessellation Issue #222](https://github.com/gfx-rs/wgpu/issues/222)
- [bevy_terrain (kurtkuehnert)](https://github.com/kurtkuehnert/bevy_terrain)
- [Frostbite Terrain Rendering](https://media.contentapi.ea.com/content/dam/eacom/frostbite/files/chapter5-andersson-terrain-rendering-in-frostbite.pdf)
- [Mapbox Terrain-RGB v1](https://docs.mapbox.com/data/tilesets/reference/mapbox-terrain-rgb-v1/)
- [Cesium Quantized Mesh Spec](https://github.com/CesiumGS/quantized-mesh)
- [MapTiler Free Terrain Tiles](https://www.maptiler.com/news/2018/08/free-terrain-tiles-for-cesium/)
