/// Bundle-aware path resolution for macOS .app bundles.
///
/// When the binary runs inside an `.app` bundle the working directory is
/// unpredictable (Finder sets it to `/`).  This module detects the bundle
/// layout and resolves asset, config, and data paths accordingly.
///
/// During development (`cargo run`) paths fall back to the project working
/// directory so existing workflows are unaffected.

use std::path::{Path, PathBuf};

/// Returns `true` when the running binary lives inside a macOS `.app` bundle
/// (i.e. the executable path contains `*.app/Contents/MacOS/`).
pub fn is_bundled() -> bool {
    bundle_contents_dir().is_some()
}

/// Returns the `Contents/` directory of the enclosing `.app` bundle, or
/// `None` when running outside a bundle.
fn bundle_contents_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    // Walk ancestors looking for Contents/MacOS
    let mut path = exe.as_path();
    loop {
        let parent = path.parent()?;
        if path.file_name().map(|n| n == "MacOS").unwrap_or(false)
            && parent.file_name().map(|n| n == "Contents").unwrap_or(false)
        {
            return Some(parent.to_path_buf());
        }
        path = parent;
    }
}

/// Base directory for locating assets and the binary.
///
/// - **Bundled**: `Contents/MacOS/` (executable's parent — Bevy resolves
///   its `assets/` folder relative to this directory).
/// - **Dev**: the current working directory.
pub fn base_dir() -> PathBuf {
    if let Some(contents) = bundle_contents_dir() {
        contents.join("MacOS")
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Assets directory (`assets/`).
///
/// - **Bundled**: `Contents/MacOS/assets/` (where Bevy's AssetPlugin looks)
/// - **Dev**: `<cwd>/assets/`
pub fn assets_dir() -> PathBuf {
    base_dir().join("assets")
}

/// Configuration directory.
///
/// - **Bundled**: `~/Library/Application Support/airjedi/`
/// - **Dev**: current working directory (preserves existing `./config.toml` behavior)
pub fn config_dir() -> PathBuf {
    if is_bundled() {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("airjedi")
    } else {
        std::env::current_dir().unwrap_or_default()
    }
}

/// Data directory for user-generated files (recordings, exports).
///
/// - **macOS**: `~/Library/Application Support/airjedi/data/`
/// - **Linux**: `~/.local/share/airjedi/data/`
/// - **Windows**: `%APPDATA%\airjedi\data\`
///
/// In development mode, falls back to `<cwd>/tmp/` to match the existing
/// convention of writing transient files there.
pub fn data_dir() -> PathBuf {
    if is_bundled() {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from(".local/share"))
            .join("airjedi")
            .join("data")
    } else {
        base_dir().join("tmp")
    }
}

/// Temporary/log directory.
///
/// - **Bundled**: `~/Library/Logs/airjedi/` (macOS) or system temp.
/// - **Dev**: `<cwd>/tmp/`
pub fn log_dir() -> PathBuf {
    if is_bundled() {
        dirs::home_dir()
            .map(|h| h.join("Library/Logs/airjedi"))
            .unwrap_or_else(|| std::env::temp_dir().join("airjedi"))
    } else {
        base_dir().join("tmp")
    }
}

/// Ensure a directory exists, creating it and all parents if necessary.
/// Returns the path unchanged for chaining.
pub fn ensure_dir(path: &Path) -> &Path {
    let _ = std::fs::create_dir_all(path);
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_bundled_returns_false_in_dev() {
        // When run via `cargo test`, we are not inside a .app bundle
        assert!(!is_bundled());
    }

    #[test]
    fn test_base_dir_falls_back_to_cwd() {
        // In dev mode, base_dir() should equal current_dir()
        let base = base_dir();
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(base, cwd);
    }

    #[test]
    fn test_assets_dir_is_under_base_dir() {
        let assets = assets_dir();
        let base = base_dir();
        assert_eq!(assets, base.join("assets"));
    }

    #[test]
    fn test_config_dir_is_cwd_in_dev() {
        // In dev mode (not bundled), config_dir() should equal current_dir()
        let config = config_dir();
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(config, cwd);
    }

    #[test]
    fn test_data_dir_is_tmp_in_dev() {
        // In dev mode (not bundled), data_dir() should be <cwd>/tmp
        let data = data_dir();
        let expected = base_dir().join("tmp");
        assert_eq!(data, expected);
    }
}
