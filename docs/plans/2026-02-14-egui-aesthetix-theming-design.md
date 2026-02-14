# egui-aesthetix Theme System Design

## Overview

Replace the current Catppuccin-only theme system with egui-aesthetix, a trait-based theming library that provides full UI skinning (colors, spacing, rounding, margins), multiple pre-built themes, support for custom theme creation, and theme persistence via config.toml.

## Goals

1. More theme variety (10 themes instead of 4)
2. Custom theme creation via Aesthetix trait implementation
3. Full UI skinning beyond colors (spacing, margins, rounding, font sizes, widget shapes)
4. Theme persistence in config.toml with restore on launch

## Architecture

### Theme Resource

The `AppTheme` Bevy resource changes from wrapping a `catppuccin::FlavorName` to holding:

- `Box<dyn Aesthetix>` — the active theme instance
- `String` — the theme's registered name (for persistence and UI display)

### Theme Registry

A `HashMap<String, fn() -> Box<dyn Aesthetix>>` populated at startup maps theme names to constructors. This allows runtime theme switching by name lookup.

### Catppuccin Adapter Themes

Four structs (`CatppuccinMocha`, `CatppuccinLatte`, `CatppuccinFrappe`, `CatppuccinMacchiato`) implement the `Aesthetix` trait using Catppuccin color palettes. This preserves the existing color choices while gaining skinning capabilities.

Mapping from Catppuccin palette to Aesthetix trait methods:

| Aesthetix method | Catppuccin color |
|---|---|
| `bg_primary_color_visuals()` | `base` |
| `bg_secondary_color_visuals()` | `mantle` |
| `bg_triage_color_visuals()` | `crust` |
| `bg_auxiliary_color_visuals()` | `surface0` |
| `bg_contrast_color_visuals()` | `surface1` |
| `primary_accent_color_visuals()` | `blue` |
| `secondary_accent_color_visuals()` | `mauve` |
| `fg_primary_text_color_visuals()` | `text` |
| `fg_success_text_color_visuals()` | `green` |
| `fg_warn_text_color_visuals()` | `yellow` |
| `fg_error_text_color_visuals()` | `red` |

Skinning values (margin, rounding, spacing, button padding, scrollbar width) use defaults that match the current app feel.

### Bevy Color Bridge

The `to_egui_color32()` and `to_egui_color32_alpha()` helpers remain. New semantic color accessor methods on `AppTheme` delegate to Aesthetix trait methods and convert to Bevy `Color`:

- `accent_primary()` -> `primary_accent_color_visuals()`
- `accent_secondary()` -> `secondary_accent_color_visuals()`
- `bg_primary()` -> `bg_primary_color_visuals()`
- `bg_secondary()` -> `bg_secondary_color_visuals()`
- `bg_auxiliary()` -> `bg_auxiliary_color_visuals()`
- `bg_contrast()` -> `bg_contrast_color_visuals()`
- `text_primary()` -> `fg_primary_text_color_visuals()`
- `text_success()` -> `fg_success_text_color_visuals()`
- `text_warn()` -> `fg_warn_text_color_visuals()`
- `text_error()` -> `fg_error_text_color_visuals()`

### egui Style Application

The `apply_egui_theme` system calls `Aesthetix::custom_style()` which produces a full `egui::Style` (including Visuals, Spacing, Interaction, text styles). This replaces the current `catppuccin_egui::set_theme()` call.

## Available Themes (10 total)

### From egui-aesthetix (6)

- Standard Dark (Adwaita-inspired)
- Standard Light (Adwaita-inspired)
- Carl Dark (KDE Plasma-inspired)
- Nord Dark
- Nord Light
- Tokyo Night Storm

### From Catppuccin adapters (4)

- Catppuccin Mocha (dark, warm)
- Catppuccin Macchiato (dark, cool)
- Catppuccin Frappe (medium)
- Catppuccin Latte (light)

## Theme Persistence

New `config.toml` section:

```toml
[appearance]
theme = "Catppuccin Mocha"
```

On startup, look up the theme name in the registry. Fall back to Catppuccin Mocha if the name is not found.

## Migration Scope

### `src/theme.rs` — Rewrite

- `AppTheme` resource holds `Box<dyn Aesthetix>` + name string
- Catppuccin-specific color accessors (`blue()`, `mantle()`, etc.) replaced with semantic names
- `apply_egui_theme` uses `Aesthetix::custom_style()` instead of `catppuccin_egui::set_theme()`
- Theme registry and Catppuccin adapter structs defined here

### 10 consuming files — Update color references

Each file calling `theme.blue()`, `theme.mantle()`, etc. updates to semantic names. Files:
- `src/main.rs`
- `src/toolbar.rs`
- `src/tools_window.rs`
- `src/config.rs`
- `src/keyboard.rs`
- `src/aircraft/stats_panel.rs`
- `src/aircraft/emergency.rs`
- `src/adsb/sync.rs`
- `src/adsb/connection.rs`

### `src/config.rs` — Add appearance config

- New `AppearanceConfig` struct with `theme: String` field
- Settings panel Theme section: dropdown listing all registered theme names
- Theme selection triggers `AppTheme` resource update

### `Cargo.toml` — Dependency changes

- Add: `egui-aesthetix` (with features for all bundled themes)
- Keep: `catppuccin` (used by adapter themes for palette colors)
- Remove: `catppuccin-egui` (replaced by Aesthetix-based style application)

## Out of Scope

- TOML-based custom theme file loading (future work)
- Live theme editor / color picker UI
- Per-widget style overrides
- Font customization via UI (Aesthetix supports it, not exposed initially)
