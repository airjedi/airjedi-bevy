# egui-aesthetix Theme System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the Catppuccin-only theme system with egui-aesthetix for full UI skinning, 10 pre-built themes, custom theme support, and persistence.

**Architecture:** AppTheme resource wraps a `Box<dyn Aesthetix + Send + Sync>` plus extended color fields for domain-specific colors not covered by the Aesthetix trait. A theme registry maps names to constructors. Four Catppuccin adapter structs implement Aesthetix using catppuccin palette colors. Config persistence via a new `[appearance]` section in config.toml.

**Tech Stack:** egui-aesthetix (git dep, main branch for egui 0.33 compat), catppuccin 2.6 (kept for adapter palette data), bevy 0.18, bevy_egui 0.39

**Design doc:** `docs/plans/2026-02-14-egui-aesthetix-theming-design.md`

---

### Task 1: Update Cargo.toml Dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Update dependencies**

Add egui-aesthetix as a git dependency (the published crate version predates egui 0.33 support, but the main branch has it). Remove catppuccin-egui. Keep catppuccin.

```toml
# Remove this line:
catppuccin-egui = { version = "5.7", default-features = false, features = ["egui33"] }

# Add this line:
egui-aesthetix = { git = "https://github.com/thebashpotato/egui-aesthetix", features = ["carl", "nord", "standard", "tokyo_night"] }
```

If egui-aesthetix has been published with 0.33 support by execution time, use a version dependency instead of git.

**Step 2: Verify dependencies resolve**

Run: `cargo check 2>&1 | head -30`
Expected: May fail on code that references catppuccin_egui — that's fine, we'll fix it in the next tasks.

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Update deps: add egui-aesthetix, remove catppuccin-egui"
```

---

### Task 2: Rewrite src/theme.rs — Core Types and Helpers

**Files:**
- Rewrite: `src/theme.rs`

This is the largest task. Replace the entire file contents.

**Step 1: Write the new theme.rs**

Key design decisions:
- `AppTheme` stores `Box<dyn Aesthetix + Send + Sync>`, a `name: String`, and extended color fields (`ext_text_dim`, `ext_bg_overlay`, `ext_altitude_low`, `ext_altitude_high`, `ext_altitude_ultra`)
- Semantic accessor methods return `bevy::color::Color` (same as current API pattern) — convert from `egui::Color32` internally
- `to_egui_color32()` / `to_egui_color32_alpha()` helpers stay unchanged
- `apply_egui_theme` system calls `Aesthetix::custom_style()` instead of `catppuccin_egui::set_theme()`

The color mapping from old to new method names:

| Old method | New method | Aesthetix source |
|---|---|---|
| `mantle()` | `bg_secondary()` | `bg_secondary_color_visuals()` |
| `surface1()` | `bg_contrast()` | `bg_contrast_color_visuals()` |
| `crust()` | `bg_triage()` | `bg_triage_color_visuals()` |
| `surface0()` | `bg_auxiliary()` | `bg_auxiliary_color_visuals()` |
| `base()` | `bg_primary()` | `bg_primary_color_visuals()` |
| `blue()` | `accent_primary()` | `primary_accent_color_visuals()` |
| `mauve()` | `accent_secondary()` | `secondary_accent_color_visuals()` |
| `text()` | `text_primary()` | `fg_primary_text_color_visuals()` |
| `subtext0()` | `text_dim()` | stored `ext_text_dim` field |
| `green()` | `text_success()` | `fg_success_text_color_visuals()` |
| `yellow()` | `text_warn()` | `fg_warn_text_color_visuals()` |
| `red()` | `text_error()` | `fg_error_text_color_visuals()` |
| `overlay0()` | `bg_overlay()` | stored `ext_bg_overlay` field |
| `teal()` | `altitude_low()` | stored `ext_altitude_low` field |
| `peach()` | `altitude_high()` | stored `ext_altitude_high` field |

Extended color defaults for non-Catppuccin themes:
- `text_dim`: blend primary text 50% toward bg_primary
- `bg_overlay`: same as bg_auxiliary
- `altitude_low`: secondary accent color
- `altitude_high`: blend warn and error 50/50
- `altitude_ultra`: secondary accent color

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui_aesthetix::Aesthetix;
use std::collections::HashMap;

// ── Conversion helpers ──────────────────────────────────────────────

/// Convert an `egui::Color32` to a `bevy::color::Color`.
fn color32_to_bevy(c: egui::Color32) -> Color {
    Color::srgba(
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    )
}

/// Convert a `bevy::color::Color` to `egui::Color32`.
pub fn to_egui_color32(color: Color) -> egui::Color32 {
    let srgba = color.to_srgba();
    egui::Color32::from_rgba_unmultiplied(
        (srgba.red * 255.0) as u8,
        (srgba.green * 255.0) as u8,
        (srgba.blue * 255.0) as u8,
        (srgba.alpha * 255.0) as u8,
    )
}

/// Convert a `bevy::color::Color` to `egui::Color32` with a custom alpha.
pub fn to_egui_color32_alpha(color: Color, alpha: u8) -> egui::Color32 {
    let srgba = color.to_srgba();
    egui::Color32::from_rgba_unmultiplied(
        (srgba.red * 255.0) as u8,
        (srgba.green * 255.0) as u8,
        (srgba.blue * 255.0) as u8,
        alpha,
    )
}

/// Blend two Color32 values by a ratio (0.0 = all `a`, 1.0 = all `b`).
fn blend_color32(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let inv = 1.0 - t;
    egui::Color32::from_rgb(
        (a.r() as f32 * inv + b.r() as f32 * t) as u8,
        (a.g() as f32 * inv + b.g() as f32 * t) as u8,
        (a.b() as f32 * inv + b.b() as f32 * t) as u8,
    )
}

// ── AppTheme resource ───────────────────────────────────────────────

/// Central theme resource for the application.
///
/// Wraps an egui-aesthetix theme and provides semantic color accessors
/// returning `bevy::color::Color` values.
#[derive(Resource)]
pub struct AppTheme {
    inner: Box<dyn Aesthetix + Send + Sync>,
    name: String,
    // Extended colors not covered by Aesthetix
    ext_text_dim: egui::Color32,
    ext_bg_overlay: egui::Color32,
    ext_altitude_low: egui::Color32,
    ext_altitude_high: egui::Color32,
    ext_altitude_ultra: egui::Color32,
}

impl AppTheme {
    /// Create a new AppTheme from an Aesthetix implementation with default extended colors.
    pub fn new(name: impl Into<String>, theme: impl Aesthetix + Send + Sync + 'static) -> Self {
        let text = theme
            .fg_primary_text_color_visuals()
            .unwrap_or(egui::Color32::WHITE);
        let bg = theme.bg_primary_color_visuals();
        let ext_text_dim = blend_color32(text, bg, 0.4);
        let ext_bg_overlay = theme.bg_auxiliary_color_visuals();
        let ext_altitude_low = theme.secondary_accent_color_visuals();
        let ext_altitude_high =
            blend_color32(theme.fg_warn_text_color_visuals(), theme.fg_error_text_color_visuals(), 0.5);
        let ext_altitude_ultra = theme.secondary_accent_color_visuals();

        Self {
            inner: Box::new(theme),
            name: name.into(),
            ext_text_dim,
            ext_bg_overlay,
            ext_altitude_low,
            ext_altitude_high,
            ext_altitude_ultra,
        }
    }

    /// Create with explicit extended colors (used by Catppuccin adapters).
    pub fn with_extended_colors(
        mut self,
        text_dim: egui::Color32,
        bg_overlay: egui::Color32,
        altitude_low: egui::Color32,
        altitude_high: egui::Color32,
        altitude_ultra: egui::Color32,
    ) -> Self {
        self.ext_text_dim = text_dim;
        self.ext_bg_overlay = bg_overlay;
        self.ext_altitude_low = altitude_low;
        self.ext_altitude_high = altitude_high;
        self.ext_altitude_ultra = altitude_ultra;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    // ── Background colors ──

    pub fn bg_primary(&self) -> Color {
        color32_to_bevy(self.inner.bg_primary_color_visuals())
    }

    pub fn bg_secondary(&self) -> Color {
        color32_to_bevy(self.inner.bg_secondary_color_visuals())
    }

    pub fn bg_triage(&self) -> Color {
        color32_to_bevy(self.inner.bg_triage_color_visuals())
    }

    pub fn bg_auxiliary(&self) -> Color {
        color32_to_bevy(self.inner.bg_auxiliary_color_visuals())
    }

    pub fn bg_contrast(&self) -> Color {
        color32_to_bevy(self.inner.bg_contrast_color_visuals())
    }

    pub fn bg_overlay(&self) -> Color {
        color32_to_bevy(self.ext_bg_overlay)
    }

    // ── Accent colors ──

    pub fn accent_primary(&self) -> Color {
        color32_to_bevy(self.inner.primary_accent_color_visuals())
    }

    pub fn accent_secondary(&self) -> Color {
        color32_to_bevy(self.inner.secondary_accent_color_visuals())
    }

    // ── Text colors ──

    pub fn text_primary(&self) -> Color {
        color32_to_bevy(
            self.inner
                .fg_primary_text_color_visuals()
                .unwrap_or(egui::Color32::WHITE),
        )
    }

    pub fn text_dim(&self) -> Color {
        color32_to_bevy(self.ext_text_dim)
    }

    pub fn text_success(&self) -> Color {
        color32_to_bevy(self.inner.fg_success_text_color_visuals())
    }

    pub fn text_warn(&self) -> Color {
        color32_to_bevy(self.inner.fg_warn_text_color_visuals())
    }

    pub fn text_error(&self) -> Color {
        color32_to_bevy(self.inner.fg_error_text_color_visuals())
    }

    // ── Domain-specific colors ──

    pub fn altitude_low(&self) -> Color {
        color32_to_bevy(self.ext_altitude_low)
    }

    pub fn altitude_high(&self) -> Color {
        color32_to_bevy(self.ext_altitude_high)
    }

    pub fn altitude_ultra(&self) -> Color {
        color32_to_bevy(self.ext_altitude_ultra)
    }
}

impl Default for AppTheme {
    fn default() -> Self {
        catppuccin_mocha()
    }
}

// ── Catppuccin adapter themes ───────────────────────────────────────

mod catppuccin_adapters;
pub use catppuccin_adapters::*;

// ── Theme registry ──────────────────────────────────────────────────

pub type ThemeConstructor = fn() -> AppTheme;

#[derive(Resource)]
pub struct ThemeRegistry {
    themes: Vec<(String, ThemeConstructor)>,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        let mut reg = Self { themes: Vec::new() };

        // Catppuccin themes
        reg.register("Catppuccin Mocha", catppuccin_mocha);
        reg.register("Catppuccin Macchiato", catppuccin_macchiato);
        reg.register("Catppuccin Frappe", catppuccin_frappe);
        reg.register("Catppuccin Latte", catppuccin_latte);

        // egui-aesthetix built-in themes
        reg.register("Standard Dark", || {
            AppTheme::new("Standard Dark", egui_aesthetix::themes::StandardDark)
        });
        reg.register("Standard Light", || {
            AppTheme::new("Standard Light", egui_aesthetix::themes::StandardLight)
        });
        reg.register("Carl Dark", || {
            AppTheme::new("Carl Dark", egui_aesthetix::themes::CarlDark)
        });
        reg.register("Nord Dark", || {
            AppTheme::new("Nord Dark", egui_aesthetix::themes::NordDark)
        });
        reg.register("Nord Light", || {
            AppTheme::new("Nord Light", egui_aesthetix::themes::NordLight)
        });
        reg.register("Tokyo Night Storm", || {
            AppTheme::new("Tokyo Night Storm", egui_aesthetix::themes::TokyoNightStorm)
        });

        reg
    }

    pub fn register(&mut self, name: &str, constructor: ThemeConstructor) {
        self.themes.push((name.to_string(), constructor));
    }

    pub fn get(&self, name: &str) -> Option<AppTheme> {
        self.themes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, ctor)| ctor())
    }

    pub fn names(&self) -> Vec<&str> {
        self.themes.iter().map(|(n, _)| n.as_str()).collect()
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── egui theme application system ───────────────────────────────────

/// System that applies the active theme's egui Style whenever `AppTheme` changes.
pub fn apply_egui_theme(theme: Res<AppTheme>, mut contexts: EguiContexts) {
    if !theme.is_changed() {
        return;
    }
    if let Ok(ctx) = contexts.ctx_mut() {
        ctx.set_style(theme.inner.custom_style());
    }
}
```

**Step 2: Create the catppuccin_adapters submodule**

Create file `src/theme/catppuccin_adapters.rs` — but wait, theme.rs is currently a single file, not a directory module. We need to restructure: rename `src/theme.rs` to `src/theme/mod.rs` and add `src/theme/catppuccin_adapters.rs`.

Alternatively, put everything in one file. Given the Catppuccin adapters are ~150 lines total, keeping it all in `src/theme.rs` with an inline module is simpler. Let's do that — put the adapter code inline at the bottom of theme.rs.

Add the following to the bottom of theme.rs (replacing the `mod catppuccin_adapters;` and `pub use` lines above with inline code):

```rust
// ── Catppuccin adapter themes ───────────────────────────────────────
//
// Each adapter implements the Aesthetix trait using catppuccin palette colors.
// Extended colors (text_dim, bg_overlay, altitude colors) use exact palette values.

use catppuccin::FlavorName;

/// Helper: get the catppuccin Color32 for a given flavor and color accessor.
fn cat_color(flavor: FlavorName, f: fn(&catppuccin::FlavorColors) -> &catppuccin::Color) -> egui::Color32 {
    let c = f(&catppuccin::PALETTE.get_flavor(flavor).colors);
    egui::Color32::from_rgb(c.rgb.r, c.rgb.g, c.rgb.b)
}

macro_rules! catppuccin_theme {
    ($struct_name:ident, $flavor:expr, $display_name:expr, $is_dark:expr) => {
        pub struct $struct_name;

        impl Aesthetix for $struct_name {
            fn name(&self) -> &str { $display_name }
            fn primary_accent_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.blue) }
            fn secondary_accent_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.mauve) }
            fn bg_primary_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.base) }
            fn bg_secondary_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.mantle) }
            fn bg_triage_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.crust) }
            fn bg_auxiliary_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.surface0) }
            fn bg_contrast_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.surface1) }
            fn fg_primary_text_color_visuals(&self) -> Option<egui::Color32> { Some(cat_color($flavor, |c| &c.text)) }
            fn fg_success_text_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.green) }
            fn fg_warn_text_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.yellow) }
            fn fg_error_text_color_visuals(&self) -> egui::Color32 { cat_color($flavor, |c| &c.red) }
            fn dark_mode_visuals(&self) -> bool { $is_dark }
            fn margin_style(&self) -> f32 { 10.0 }
            fn button_padding(&self) -> egui::Vec2 { egui::Vec2::new(8.0, 4.0) }
            fn item_spacing_style(&self) -> f32 { 8.0 }
            fn scroll_bar_width_style(&self) -> f32 { 12.0 }
            fn rounding_visuals(&self) -> f32 { 6.0 }
        }

        pub fn $struct_name() -> AppTheme {
            let flavor = $flavor;
            AppTheme::new($display_name, $struct_name).with_extended_colors(
                cat_color(flavor, |c| &c.subtext0),   // text_dim
                cat_color(flavor, |c| &c.overlay0),    // bg_overlay
                cat_color(flavor, |c| &c.teal),        // altitude_low
                cat_color(flavor, |c| &c.peach),       // altitude_high
                cat_color(flavor, |c| &c.mauve),       // altitude_ultra
            )
        }
    };
}

catppuccin_theme!(CatppuccinMocha, FlavorName::Mocha, "Catppuccin Mocha", true);
catppuccin_theme!(CatppuccinMacchiato, FlavorName::Macchiato, "Catppuccin Macchiato", true);
catppuccin_theme!(CatppuccinFrappe, FlavorName::Frappe, "Catppuccin Frappe", true);
catppuccin_theme!(CatppuccinLatte, FlavorName::Latte, "Catppuccin Latte", false);

// Constructor functions for the registry (lowercase names matching the macro-generated structs)
pub fn catppuccin_mocha() -> AppTheme { CatppuccinMocha() }
pub fn catppuccin_macchiato() -> AppTheme { CatppuccinMacchiato() }
pub fn catppuccin_frappe() -> AppTheme { CatppuccinFrappe() }
pub fn catppuccin_latte() -> AppTheme { CatppuccinLatte() }
```

Note: The macro generates both a struct name and a function name with the same identifier. This may cause a naming conflict. Use lowercase function names instead:

```rust
catppuccin_theme!(CatppuccinMochaTheme, FlavorName::Mocha, "Catppuccin Mocha", true);
// ... etc.
pub fn catppuccin_mocha() -> AppTheme { CatppuccinMochaTheme() }
```

Adjust the macro's constructor function name to avoid collision with the struct name. The exact naming is up to the implementer — the key requirement is that each flavor produces an `AppTheme` with the correct extended colors from the catppuccin palette.

**Step 3: Verify theme.rs compiles in isolation**

Run: `cargo check 2>&1 | head -50`
Expected: Errors in other files referencing old API (`.blue()`, `.mantle()`, etc.) — that's expected and addressed in later tasks.

**Step 4: Commit**

```bash
git add src/theme.rs
git commit -m "Rewrite theme.rs with egui-aesthetix and Catppuccin adapters"
```

---

### Task 3: Update Config for Theme Persistence

**Files:**
- Modify: `src/config.rs`

**Step 1: Add AppearanceConfig to the config structs**

Add a new config section and field:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppearanceConfig {
    pub theme: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: "Catppuccin Mocha".to_string(),
        }
    }
}
```

Add to `AppConfig`:

```rust
pub struct AppConfig {
    // ... existing fields ...
    #[serde(default)]
    pub appearance: AppearanceConfig,
}
```

Update `AppConfig::default()` to include `appearance: AppearanceConfig::default()`.

**Step 2: Update the settings panel Theme section**

Replace the current Catppuccin flavor dropdown (lines 350-365 in config.rs) with a theme name dropdown that uses the `ThemeRegistry`:

```rust
// In render_settings_panel, add ThemeRegistry as a system parameter:
// theme_registry: Res<ThemeRegistry>,

// Replace the Theme collapsing section:
ui.collapsing("Theme", |ui| {
    let current_name = app_theme.name().to_string();
    egui::ComboBox::from_id_salt("theme_selector")
        .selected_text(&current_name)
        .show_ui(ui, |ui| {
            for name in theme_registry.names() {
                let selected = current_name == name;
                if ui.selectable_label(selected, name).clicked() {
                    if let Some(new_theme) = theme_registry.get(name) {
                        *app_theme = new_theme;
                    }
                }
            }
        });
});
```

Add `ThemeRegistry` as a parameter to `render_settings_panel`:

```rust
pub fn render_settings_panel(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<SettingsUiState>,
    mut app_config: ResMut<AppConfig>,
    mut app_theme: ResMut<AppTheme>,
    theme_registry: Res<ThemeRegistry>,
) {
```

**Step 3: Save and load theme name**

When saving config, capture the current theme name:

In the Save button handler, before `save_config(&new_config)`, set:
```rust
new_config.appearance.theme = app_theme.name().to_string();
```

Remove all imports of `crate::theme::{self, AppTheme}` flavor-related items (`ALL_FLAVORS`, `flavor_display_name`, `FlavorName`). Import `ThemeRegistry` instead:

```rust
use crate::theme::{AppTheme, ThemeRegistry};
```

**Step 4: Commit**

```bash
git add src/config.rs
git commit -m "Update config for theme persistence with ThemeRegistry"
```

---

### Task 4: Update main.rs Initialization

**Files:**
- Modify: `src/main.rs`

**Step 1: Initialize ThemeRegistry and load theme from config**

Replace `.init_resource::<theme::AppTheme>()` with:

```rust
.init_resource::<theme::ThemeRegistry>()
```

And after the config is loaded (after `.insert_resource(config)`), insert the AppTheme based on the config's appearance.theme value. This requires loading the theme from the registry at startup. Add an initialization system or do it inline:

```rust
// After config is loaded, before .init_resource::<theme::ThemeRegistry>():
let registry = theme::ThemeRegistry::new();
let initial_theme = registry
    .get(&config.appearance.theme)
    .unwrap_or_else(|| registry.get("Catppuccin Mocha").unwrap());

// ...
.insert_resource(registry)
.insert_resource(initial_theme)
```

The `theme::apply_egui_theme` system registration stays the same.

**Step 2: Verify it compiles**

Run: `cargo check 2>&1 | head -50`
Expected: Errors in consuming files that still reference old color methods. These are fixed in the next tasks.

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Initialize ThemeRegistry and load theme from config"
```

---

### Task 5: Update egui Panel Files

**Files:**
- Modify: `src/toolbar.rs`
- Modify: `src/tools_window.rs`
- Modify: `src/aircraft/stats_panel.rs`

These files use `theme.xxx()` with `to_egui_color32()` / `to_egui_color32_alpha()` for egui UI rendering. Apply these renames:

| Old call | New call |
|---|---|
| `theme.mantle()` | `theme.bg_secondary()` |
| `theme.surface1()` | `theme.bg_contrast()` |
| `theme.blue()` | `theme.accent_primary()` |
| `theme.subtext0()` | `theme.text_dim()` |
| `theme.text()` | `theme.text_primary()` |
| `theme.crust()` | `theme.bg_triage()` |
| `theme.overlay0()` | `theme.bg_overlay()` |
| `theme.green()` | `theme.text_success()` |
| `theme.yellow()` | `theme.text_warn()` |
| `theme.red()` | `theme.text_error()` |
| `theme.teal()` | `theme.altitude_low()` |
| `theme.peach()` | `theme.altitude_high()` |
| `theme.mauve()` | `theme.accent_secondary()` |

**Step 1: Update toolbar.rs**

Specific changes:
- Line 36: `theme.mantle()` → `theme.bg_secondary()`
- Line 37: `theme.surface1()` → `theme.bg_contrast()`
- Line 50: `theme.blue()` → `theme.accent_primary()`
- Line 51: `theme.subtext0()` → `theme.text_dim()`
- Line 52: `theme.blue()` → `theme.accent_primary()`
- Line 87: `theme.subtext0()` → `theme.text_dim()`
- Line 197: `theme.overlay0()` → `theme.bg_overlay()`
- Line 208: `theme.green()` → `theme.text_success()`
- Line 212: `theme.yellow()` → `theme.text_warn()`
- Line 216: `theme.red()` → `theme.text_error()`
- Line 220: `theme.red()` → `theme.text_error()`
- Line 243: `theme.crust()` → `theme.bg_triage()`
- Line 249: `theme.subtext0()` → `theme.text_dim()`

**Step 2: Update tools_window.rs**

Specific changes:
- Line 59: `theme.mantle()` → `theme.bg_secondary()`
- Line 60: `theme.surface1()` → `theme.bg_contrast()`

**Step 3: Update stats_panel.rs**

Specific changes:
- Line 102: `theme.mantle()` → `theme.bg_secondary()`
- Line 103: `theme.surface1()` → `theme.bg_contrast()`
- Line 104: `theme.blue()` → `theme.accent_primary()`
- Line 105: `theme.subtext0()` → `theme.text_dim()`
- Line 106: `theme.text()` → `theme.text_primary()`
- Line 107: `theme.teal()` → `theme.altitude_low()`
- Line 108: `theme.yellow()` → `theme.text_warn()`
- Line 109: `theme.peach()` → `theme.altitude_high()`
- Line 110: `theme.mauve()` → `theme.accent_secondary()`
- Line 206: `theme.overlay0()` → `theme.bg_overlay()`

**Step 4: Commit**

```bash
git add src/toolbar.rs src/tools_window.rs src/aircraft/stats_panel.rs
git commit -m "Update egui panel files to use semantic theme colors"
```

---

### Task 6: Update Bevy Rendering Files

**Files:**
- Modify: `src/adsb/connection.rs`
- Modify: `src/adsb/sync.rs`
- Modify: `src/aircraft/emergency.rs`
- Modify: `src/keyboard.rs`

These files use `theme.xxx()` returning `bevy::color::Color` directly (no egui conversion).

**Step 1: Update adsb/connection.rs**

- Line 137: `theme.green()` → `theme.text_success()`
- Line 141: `theme.yellow()` → `theme.text_warn()`
- Line 145: `theme.red()` → `theme.text_error()`
- Line 149: `theme.red()` → `theme.text_error()`

**Step 2: Update adsb/sync.rs**

- Line 116: `theme.text()` → `theme.text_primary()`

**Step 3: Update aircraft/emergency.rs**

- Line 174: `theme.red()` → `theme.text_error()`

**Step 4: Update keyboard.rs**

- Line 380: `theme.mantle()` → `theme.bg_secondary()`
- Line 381: `theme.text()` → `theme.text_primary()`

**Step 5: Commit**

```bash
git add src/adsb/connection.rs src/adsb/sync.rs src/aircraft/emergency.rs src/keyboard.rs
git commit -m "Update Bevy rendering files to use semantic theme colors"
```

---

### Task 7: Build Verification and Cleanup

**Files:**
- Possibly any file with remaining compilation errors

**Step 1: Full build**

Run: `cargo build 2>&1`
Expected: Clean compilation. If errors, fix them — likely from:
- Missing imports (remove old `catppuccin_egui` imports, add `egui_aesthetix`)
- Trait method signature mismatches (check Aesthetix trait against the exact version on main branch)
- Send/Sync bounds on Box<dyn Aesthetix>

**Step 2: Verify the macro-generated code compiles**

The catppuccin adapter macro generates structs and Aesthetix impls. If the macro has issues, flatten it into explicit struct/impl blocks for each flavor.

**Step 3: Run the application**

Run: `cargo run`
Expected: Application starts with Catppuccin Mocha theme. Open Settings (Esc), verify:
- Theme dropdown shows all 10 themes
- Switching themes updates egui panel colors immediately
- Switching themes updates Bevy-side colors (aircraft labels, status text, emergency banner)
- No panics or visual glitches

**Step 4: Test theme persistence**

1. Switch to "Nord Dark" in settings, click Save
2. Close and relaunch: `cargo run`
3. Verify app starts with Nord Dark theme
4. Check config.toml has `[appearance]\ntheme = "Nord Dark"`

**Step 5: Final commit**

```bash
git add -A
git commit -m "Fix remaining compilation issues from theme migration"
```

Only create this commit if there were fixes needed. If Task 6 compiled clean, skip this.

---

### Task 8: Cleanup Dead Code

**Files:**
- Modify: `src/theme.rs` (if any old code remains)
- Modify: `Cargo.toml` (verify catppuccin-egui is removed)

**Step 1: Verify no references to old API**

Search for any remaining references to the old Catppuccin-specific API:

```bash
grep -rn "flavor\|ALL_FLAVORS\|flavor_display_name\|egui_theme\|catppuccin_egui\|set_flavor\|FlavorName" src/
```

Expected: No matches except in the catppuccin adapter code's use of `FlavorName`.

**Step 2: Verify catppuccin-egui is fully removed**

```bash
grep "catppuccin-egui\|catppuccin_egui" Cargo.toml Cargo.lock
```

Expected: No matches in Cargo.toml. May appear in Cargo.lock as a transitive dep — that's fine, but shouldn't be a direct dependency.

**Step 3: Commit if any cleanup was needed**

```bash
git add -A
git commit -m "Remove dead catppuccin-egui references"
```
