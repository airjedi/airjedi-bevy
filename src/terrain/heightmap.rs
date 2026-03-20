//! Heightmap tile fetching, decoding, and caching.

use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use std::collections::{HashMap, HashSet};

use super::provider::TerrainProvider;

/// Key for a terrain tile: (zoom, x, y)
pub(crate) type TileKey = (u8, u32, u32);

/// Decoded heightmap data for a single terrain tile.
/// Contains a 256x256 grid of elevation values in meters.
pub(crate) struct HeightmapData {
    width: u32,
    height: u32,
    elevations: Vec<f32>,
    pub min_elevation: f32,
    pub max_elevation: f32,
}

impl HeightmapData {
    /// Width of the heightmap grid in pixels.
    pub(crate) fn width(&self) -> usize {
        self.width as usize
    }

    /// Height of the heightmap grid in pixels.
    pub(crate) fn height(&self) -> usize {
        self.height as usize
    }

    /// Sample the elevation at pixel coordinates (x, y) in meters.
    /// Returns 0.0 for out-of-bounds coordinates.
    pub(crate) fn elevation(&self, x: usize, y: usize) -> f32 {
        if x < self.width as usize && y < self.height as usize {
            self.elevations[y * self.width as usize + x]
        } else {
            0.0
        }
    }
}

/// Resource that caches decoded heightmaps and manages async fetch requests.
#[derive(Resource)]
pub(crate) struct HeightmapCache {
    cache: HashMap<TileKey, HeightmapData>,
    pending: HashSet<TileKey>,
    sender: Sender<(TileKey, HeightmapData)>,
    receiver: Receiver<(TileKey, HeightmapData)>,
    pub provider: TerrainProvider,
}

impl HeightmapCache {
    /// Create a new heightmap cache with the given elevation data provider.
    pub(crate) fn new(provider: TerrainProvider) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self {
            cache: HashMap::new(),
            pending: HashSet::new(),
            sender,
            receiver,
            provider,
        }
    }

    /// Returns a reference to the cached heightmap for the given tile key, if present.
    pub(crate) fn get(&self, key: &TileKey) -> Option<&HeightmapData> {
        self.cache.get(key)
    }

    /// Returns true if the tile is already cached or has a pending fetch request.
    pub(crate) fn contains(&self, key: &TileKey) -> bool {
        self.cache.contains_key(key) || self.pending.contains(key)
    }

    /// Spawn an async fetch for the given tile if it is not already cached or pending.
    pub(crate) fn request(&mut self, key: TileKey) {
        if self.cache.contains_key(&key) || self.pending.contains(&key) {
            return;
        }
        self.pending.insert(key);
        fetch_and_decode(self.provider.clone(), key, self.sender.clone());
    }

    /// Drain the receiver channel, inserting all completed heightmaps into the cache.
    pub(crate) fn poll_completed(&mut self) {
        while let Ok((key, data)) = self.receiver.try_recv() {
            self.pending.remove(&key);
            self.cache.insert(key, data);
        }
    }

    /// Remove a cached heightmap (for tile eviction).
    pub(crate) fn remove(&mut self, key: &TileKey) {
        self.cache.remove(key);
        self.pending.remove(key);
    }

    /// Evict heightmaps outside the active zoom band and enforce a size cap.
    /// Returns the number of entries removed.
    pub(crate) fn evict(&mut self, current_zoom: u8, max_entries: usize) -> usize {
        let mut removed = 0;

        // Phase 1: Remove entries outside the active zoom band [current-4, current]
        let min_zoom = current_zoom.saturating_sub(4);
        self.cache.retain(|&(zoom, _, _), _| {
            let keep = zoom >= min_zoom && zoom <= current_zoom;
            if !keep { removed += 1; }
            keep
        });
        self.pending.retain(|&(zoom, _, _)| zoom >= min_zoom && zoom <= current_zoom);

        // Phase 2: If still over budget, drop oldest (arbitrary) entries
        if self.cache.len() > max_entries {
            let excess = self.cache.len() - max_entries;
            let keys_to_remove: Vec<TileKey> = self.cache.keys().take(excess).copied().collect();
            for key in keys_to_remove {
                self.cache.remove(&key);
                removed += 1;
            }
        }

        removed
    }

    /// Number of cached heightmaps.
    pub(crate) fn len(&self) -> usize {
        self.cache.len()
    }
}

/// Spawn a background thread to fetch an elevation tile PNG, decode it, and
/// send the resulting heightmap data back through the channel.
fn fetch_and_decode(
    provider: TerrainProvider,
    key: TileKey,
    sender: Sender<(TileKey, HeightmapData)>,
) {
    let (zoom, x, y) = key;
    let url = provider.tile_url(zoom, x, y);

    std::thread::spawn(move || {
        let bytes = match reqwest::blocking::get(&url).and_then(|r| r.bytes()) {
            Ok(b) => b,
            Err(_) => return,
        };
        let img = match image::load_from_memory(&bytes) {
            Ok(i) => i.to_rgb8(),
            Err(_) => return,
        };
        let (w, h) = (img.width(), img.height());
        let mut elevations = Vec::with_capacity((w * h) as usize);
        let mut min_elev = f32::MAX;
        let mut max_elev = f32::MIN;
        for pixel in img.pixels() {
            let elev = provider.decode_elevation(pixel[0], pixel[1], pixel[2]);
            min_elev = min_elev.min(elev);
            max_elev = max_elev.max(elev);
            elevations.push(elev);
        }
        let _ = sender.send((key, HeightmapData {
            width: w,
            height: h,
            elevations,
            min_elevation: min_elev,
            max_elevation: max_elev,
        }));
    });
}

// ---------------------------------------------------------------------------
// Bevy systems
// ---------------------------------------------------------------------------

/// System that polls the heightmap completion channel each frame, inserting
/// finished heightmaps into the cache.
pub(crate) fn poll_heightmap_completions(mut cache: ResMut<HeightmapCache>) {
    cache.poll_completed();
}

/// System that requests heightmap fetches for visible map tiles in 3D mode.
///
/// Runs after tile spawning. Iterates over spawned tile entities that have both
/// a `MapTile` marker and a `TileFadeState`. For each tile at the current zoom
/// level, computes slippy tile coordinates from the tile's world-space transform
/// position and requests the corresponding heightmap from the cache.
pub(crate) fn request_heightmaps_for_tiles(
    view_state: Res<crate::view3d::View3DState>,
    terrain_state: Res<super::TerrainState>,
    map_state: Res<crate::MapState>,
    mut cache: ResMut<HeightmapCache>,
    tile_settings: Res<bevy_slippy_tiles::SlippyTilesSettings>,
    tile_query: Query<
        (&Transform, &crate::tiles::TileFadeState),
        With<bevy_slippy_tiles::MapTile>,
    >,
) {
    // Only fetch heightmaps when 3D mode is active and terrain is enabled
    if !view_state.is_3d_active() || !terrain_state.enabled {
        return;
    }

    let current_zoom = map_state.zoom_level.to_u8();

    for (transform, fade_state) in tile_query.iter() {
        if fade_state.tile_zoom != current_zoom {
            continue;
        }

        let key = super::tile_key_from_transform(transform, fade_state, &tile_settings, map_state.zoom_level);

        if !cache.contains(&key) {
            cache.request(key);
        }
    }
}
