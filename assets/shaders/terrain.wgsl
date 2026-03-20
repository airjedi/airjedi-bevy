// Terrain vertex displacement + emissive fragment shader.
//
// Vertex stage: samples a Terrarium-encoded heightmap texture and displaces
// vertices vertically. Computes surface normals via central finite differences.
//
// Fragment stage: renders the satellite tile texture as emissive (bypassing
// HDR camera exposure) with a subtle directional shading term derived from
// the terrain normal to give visual depth.

#import bevy_pbr::{
    mesh_bindings::mesh,
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

// --- Material bind group (group 1) ---

struct TerrainParams {
    elevation_scale: f32,
    texel_size: f32,
    emissive_boost: f32,
    transition_factor: f32,
}

@group(1) @binding(0) var<uniform> params: TerrainParams;
@group(1) @binding(1) var satellite_texture: texture_2d<f32>;
@group(1) @binding(2) var satellite_sampler: sampler;
@group(1) @binding(3) var heightmap_texture: texture_2d<f32>;
@group(1) @binding(4) var heightmap_sampler: sampler;

// Decode Terrarium PNG pixel to elevation in meters.
// Terrarium encoding: height = R*256 + G + B/256 - 32768
fn decode_terrarium(color: vec4<f32>) -> f32 {
    // Color channels are 0.0-1.0 in the shader, so scale to 0-255.
    let r = color.r * 255.0;
    let g = color.g * 255.0;
    let b = color.b * 255.0;
    return r * 256.0 + g + b / 256.0 - 32768.0;
}

// Sample elevation at a UV coordinate.
fn sample_elevation(uv: vec2<f32>) -> f32 {
    let color = textureSampleLevel(heightmap_texture, heightmap_sampler, uv, 0.0);
    return decode_terrarium(color);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var vertex = vertex_in;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

    // Sample heightmap at vertex UV and displace Y
    let elevation_m = sample_elevation(vertex.uv);
    let displacement = elevation_m * 0.001 * params.elevation_scale * params.transition_factor;
    vertex.position.y += displacement;

    // Compute normal via central finite differences on the heightmap
    let ts = params.texel_size;
    let h_left  = sample_elevation(vertex.uv + vec2<f32>(-ts, 0.0));
    let h_right = sample_elevation(vertex.uv + vec2<f32>( ts, 0.0));
    let h_down  = sample_elevation(vertex.uv + vec2<f32>(0.0, -ts));
    let h_up    = sample_elevation(vertex.uv + vec2<f32>(0.0,  ts));

    let scale = 0.001 * params.elevation_scale * params.transition_factor;
    let dh_dx = (h_right - h_left) * scale;
    let dh_dz = (h_up - h_down) * scale;

    let terrain_normal = normalize(vec3<f32>(-dh_dx, 2.0 * ts, -dh_dz));

    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        terrain_normal,
        vertex.instance_index,
    );

    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
    out.position = position_world_to_clip(out.world_position.xyz);

    out.uv = vertex.uv;

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let satellite_color = textureSample(satellite_texture, satellite_sampler, in.uv);

    // Subtle directional shading: dot product with a fixed sun direction
    // adds visual depth without relying on the PBR lighting pipeline.
    let sun_dir = normalize(vec3<f32>(0.3, 0.8, 0.4));
    let ndotl = max(dot(normalize(in.world_normal), sun_dir), 0.0);
    let shading = mix(0.7, 1.0, ndotl); // 70% ambient, 30% directional

    // Emissive output: multiply by boost factor for HDR-independent brightness
    let emissive = satellite_color.rgb * params.emissive_boost * shading;

    return vec4<f32>(emissive, 1.0);
}
