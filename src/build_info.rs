use shadow_rs::shadow;

shadow!(build);

/// Short version string for the status bar: "v0.1.0-af6869e"
pub fn version_short() -> String {
    let hash = &build::SHORT_COMMIT[..7.min(build::SHORT_COMMIT.len())];
    let dirty = if build::GIT_CLEAN { "" } else { "*" };
    format!("v{}-{}{}", build::PKG_VERSION, hash, dirty)
}
