/// Centralized tile cache management.
///
/// Stores map tiles in the platform cache directory (`~/Library/Caches/airjedi/tiles`
/// on macOS) and symlinks them into the Bevy assets directory so the AssetServer
/// can load them transparently.

use bevy::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Returns the platform-appropriate tile cache directory.
///
/// - macOS:   `~/Library/Caches/airjedi/tiles`
/// - Linux:   `~/.cache/airjedi/tiles`
/// - Windows: `%LOCALAPPDATA%\airjedi\cache\tiles`
pub fn tile_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("airjedi")
        .join("tiles")
}

/// Ensures the centralized cache directory exists and is symlinked into
/// `assets/tiles` so bevy_slippy_tiles can read/write through the AssetServer.
///
/// Called once at startup before any tile downloads.
pub fn setup_tile_cache() {
    let cache_dir = tile_cache_dir();

    // Create the cache directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&cache_dir) {
        warn!("Failed to create tile cache directory {:?}: {}", cache_dir, e);
        return;
    }

    // Determine the assets/tiles symlink path
    let assets_tiles = assets_tiles_path();

    // If assets/tiles already exists, check if it's correct
    if assets_tiles.exists() || assets_tiles.symlink_metadata().is_ok() {
        if assets_tiles.symlink_metadata().is_ok() {
            if let Ok(target) = fs::read_link(&assets_tiles) {
                if target == cache_dir {
                    info!("Tile cache symlink already correct: {:?} -> {:?}", assets_tiles, cache_dir);
                    return;
                }
                // Symlink points elsewhere, remove and recreate
                warn!("Tile cache symlink points to {:?}, updating to {:?}", target, cache_dir);
                let _ = fs::remove_file(&assets_tiles);
            } else {
                // It's a real directory, not a symlink — migrate any existing tiles
                migrate_existing_tiles(&assets_tiles, &cache_dir);
            }
        }
    }

    // Ensure the assets directory exists
    let assets_dir = assets_tiles.parent().unwrap_or(Path::new("assets"));
    if let Err(e) = fs::create_dir_all(assets_dir) {
        warn!("Failed to create assets directory {:?}: {}", assets_dir, e);
        return;
    }

    // Create the symlink
    #[cfg(unix)]
    {
        if let Err(e) = std::os::unix::fs::symlink(&cache_dir, &assets_tiles) {
            warn!("Failed to create tile cache symlink {:?} -> {:?}: {}", assets_tiles, cache_dir, e);
            return;
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = std::os::windows::fs::symlink_dir(&cache_dir, &assets_tiles) {
            warn!("Failed to create tile cache symlink {:?} -> {:?}: {}", assets_tiles, cache_dir, e);
            return;
        }
    }

    info!("Tile cache: {:?} -> {:?}", assets_tiles, cache_dir);
}

/// Clears all cached tile files from the centralized cache directory.
pub fn clear_tile_cache() {
    let cache_dir = tile_cache_dir();

    if !cache_dir.exists() {
        warn!("Tile cache directory not found at {:?}", cache_dir);
        return;
    }

    let mut deleted_count = 0;

    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.contains(".tile.") {
                        if let Err(e) = fs::remove_file(&path) {
                            warn!("Failed to delete tile {:?}: {}", path, e);
                        } else {
                            deleted_count += 1;
                        }
                    }
                }
            }
        }
    }

    info!("Cleared {} tile(s) from cache at {:?}", deleted_count, cache_dir);
}

/// Also clear any legacy tiles sitting directly in `assets/` from before
/// the centralized cache was introduced.
pub fn clear_legacy_tiles() {
    let assets_path = std::env::current_dir()
        .map(|path| path.join("assets"))
        .unwrap_or_else(|_| PathBuf::from("assets"));

    if !assets_path.exists() {
        return;
    }

    let mut deleted_count = 0;
    if let Ok(entries) = fs::read_dir(&assets_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.contains(".tile.") {
                        if let Err(e) = fs::remove_file(&path) {
                            warn!("Failed to delete legacy tile {:?}: {}", path, e);
                        } else {
                            deleted_count += 1;
                        }
                    }
                }
            }
        }
    }

    if deleted_count > 0 {
        info!("Cleared {} legacy tile(s) from assets/", deleted_count);
    }
}

/// Remove cached tiles whose file content doesn't match their extension.
/// Invalid tiles cause Bevy's asset loader to log errors every frame.
pub fn remove_invalid_tiles() {
    let cache_dir = tile_cache_dir();
    if !cache_dir.exists() {
        return;
    }

    let png_signature: [u8; 4] = [0x89, 0x50, 0x4E, 0x47]; // \x89PNG
    let jpg_signature: [u8; 2] = [0xFF, 0xD8];              // JPEG SOI
    let mut removed = 0;

    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // Determine expected format from file extension
            let expected = if filename.ends_with(".tile.png") {
                "png"
            } else if filename.ends_with(".tile.jpg") {
                "jpg"
            } else if filename.ends_with(".tile.webp") {
                "webp"
            } else {
                continue;
            };
            // Read first bytes and validate against expected format
            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(_) => {
                    let _ = fs::remove_file(&path);
                    removed += 1;
                    continue;
                }
            };
            let valid = match expected {
                "png" => bytes.len() >= 4 && bytes[..4] == png_signature,
                "jpg" => bytes.len() >= 2 && bytes[..2] == jpg_signature,
                "webp" => bytes.len() >= 4 && &bytes[..4] == b"RIFF",
                _ => true,
            };
            if !valid {
                let sig = if bytes.len() >= 4 {
                    format!("{:02x}{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2], bytes[3])
                } else {
                    format!("({} bytes)", bytes.len())
                };
                warn!("Removing invalid tile {} (signature: {})", filename, sig);
                let _ = fs::remove_file(&path);
                removed += 1;
            }
        }
    }

    if removed > 0 {
        info!("Removed {} invalid tile(s) from cache", removed);
    }
}

fn assets_tiles_path() -> PathBuf {
    std::env::current_dir()
        .map(|path| path.join("assets").join("tiles"))
        .unwrap_or_else(|_| PathBuf::from("assets/tiles"))
}

/// Move existing tiles from a real `assets/tiles/` directory into the cache,
/// then remove the directory so it can be replaced with a symlink.
fn migrate_existing_tiles(source: &Path, dest: &Path) {
    info!("Migrating existing tiles from {:?} to {:?}", source, dest);
    let mut migrated = 0;

    if let Ok(entries) = fs::read_dir(source) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    let dest_path = dest.join(filename);
                    if let Err(e) = fs::rename(&path, &dest_path) {
                        warn!("Failed to migrate tile {:?}: {}", path, e);
                    } else {
                        migrated += 1;
                    }
                }
            }
        }
    }

    // Remove the now-empty directory
    let _ = fs::remove_dir(source);
    info!("Migrated {} tile(s) to centralized cache", migrated);
}
