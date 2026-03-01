# Custom egui Widget Library Design

## Problem

The current UI uses standard egui primitives (Button, Label, Grid, etc.) with flat backgrounds and no visual depth effects. The aircraft detail panel and dock panes lack the polished, professional appearance needed for an aviation application. There is no way to apply drop shadows, gradient backgrounds, or accent glow effects to UI elements using the current setup.

## Goals

1. Gradient backgrounds for dock panels to add visual depth
2. Reusable custom widgets for aircraft detail display with special visual styling
3. Theme-integrated effects — all colors derive from the current Aesthetix theme (accent, border, highlight)
4. Natural dark drop shadows and accent-colored glow effects
5. Hybrid card + gauge layout for aircraft details

## Approach: Pure egui Painter API

Use egui's built-in rendering capabilities:
- `egui::Shadow` with `RectShape::with_blur_width()` for drop shadows and glow (GPU-accelerated, native to egui 0.33.3)
- `epaint::Mesh` with per-vertex `Color32` for gradient backgrounds (same technique as egui's color picker)
- `epaint::PathShape::line()` with trigonometric arc generation for circular gauges

No custom shaders, no bevy_egui modifications, no separate render layers. The widget API abstracts over the rendering so a GPU shader backend could be swapped in later without changing call sites.

### Alternatives Considered

- **Bevy render layer overlay**: Real GPU blur via Bevy sprites, but complex position synchronization between egui layout and Bevy entities every frame. Breaks widget encapsulation.
- **PaintCallback shader**: Full shader control inside egui pipeline, but requires deep bevy_egui render pipeline integration and has high maintenance burden across upgrades.

## Module Structure

```
src/widgets/
  mod.rs              # Re-exports, WidgetTheme helper
  shadow_frame.rs     # ShadowFrame — Frame with configurable drop shadow + glow
  gradient_panel.rs   # GradientPanel — vertical/horizontal gradient backgrounds
  card.rs             # Card — rounded container combining shadow + gradient + border
  gauge.rs            # ArcGauge — circular/arc gauge for flight metrics
  data_strip.rs       # DataStrip — horizontal data ribbon with accent border
  effects.rs          # Low-level helpers: gradient mesh, arc points
```

## WidgetTheme

A lightweight struct extracted from `AppTheme` carrying the colors widgets need:

```rust
pub struct WidgetTheme {
    pub bg_primary: Color32,
    pub bg_secondary: Color32,
    pub accent: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub shadow_color: Color32,  // typically black @ 40-60 alpha
}
```

Implements `From<&AppTheme>` to bridge the existing theme system. Widgets accept `&WidgetTheme` so they have no dependency on the full `AppTheme` resource or Bevy ECS.

## Widget Specifications

### ShadowFrame

Wraps `egui::Frame` with configurable shadow/glow presets.

```rust
ShadowFrame::new(theme)
    .shadow(ShadowPreset::Subtle)     // Subtle, Medium, Strong, or Custom
    .glow(accent_color, 16)           // optional accent glow with blur radius
    .corner_radius(8)
    .show(ui, |ui| { ... });
```

Shadow presets map to `egui::Shadow` values:
- **Subtle**: offset [0,2], blur 8, spread 0, black alpha 40
- **Medium**: offset [0,4], blur 12, spread 0, black alpha 60
- **Strong**: offset [2,6], blur 20, spread 2, black alpha 80

Glow uses a centered shadow (offset [0,0]) with the accent color at reduced alpha and configurable blur radius.

### GradientPanel

Paints a vertex-colored mesh as a panel background.

```rust
GradientPanel::vertical(top_color, bottom_color)
    .corner_radius(8)
    .show(ui, |ui| { ... });
```

- Builds an `epaint::Mesh` with 4 vertices (2-color) or 2*(N+1) vertices (multi-stop)
- Corner rounding masked with a border stroke (egui has no per-mesh clip to rounded rect)
- Directions: `vertical()`, `horizontal()`, `multi(colors)`

### Card

Composes ShadowFrame + optional gradient header + content area.

```rust
Card::new(theme)
    .header("Aircraft Info", Some(icon))
    .gradient_header(theme.accent, theme.bg_secondary)
    .shadow(ShadowPreset::Medium)
    .show(ui, |ui| { ... });
```

- Header bar gets a horizontal gradient background
- Body gets a solid fill from theme
- Shadow wraps the entire card
- Rounded corners on all four corners

### ArcGauge

Implements `egui::Widget` for a circular/arc gauge.

```rust
ui.add(ArcGauge::new(0.73)
    .size(100.0)
    .label("ALT")
    .value_text("FL350")
    .track_color(theme.bg_secondary)
    .fill_color(theme.accent)
    .sweep(270.0));
```

- Track arc: `PathShape::line()` with trig-generated points (64 segments)
- Value arc: Same technique, clipped to value proportion
- Tick marks: `Painter::line_segment()` at regular intervals
- Center label: `Painter::text()` with proportional font sizing
- Thick arc variant via `Mesh` for filled arc bands with optional gradient

### DataStrip

Horizontal information ribbon with accent border.

```rust
DataStrip::new(theme)
    .accent_border_left(theme.accent, 3.0)
    .glow(theme.accent, 8)
    .show(ui, |ui| {
        ui.label("HDG");
        ui.label("270deg");
    });
```

- Compact horizontal layout with left-edge colored accent bar
- Optional glow effect on the accent edge
- Stacks vertically for dense data display

### Effects Module (effects.rs)

Low-level building blocks used by widgets internally, also public for one-off custom painting:

- `paint_gradient_rect(painter, rect, colors, direction)` — mesh construction and painting
- `paint_arc(painter, center, radius, start, end, stroke, segments)` — arc point generation + PathShape
- `paint_thick_arc(painter, center, inner_r, outer_r, start, end, color, segments)` — filled arc band via Mesh
- `gradient_color(start, end, t)` — linear color interpolation in sRGB

## Integration Points

### Aircraft Detail Panel

Replace the current flat layout in `src/aircraft/detail_panel.rs` with:
- **Identity card**: Card with gradient header showing callsign, airline, aircraft type
- **Position card**: Card with lat/lon/altitude/heading data strips
- **Performance gauges**: ArcGauge widgets for altitude, speed, heading, vertical rate
- **Flight info card**: Origin/destination, squawk, category

Selected/focused aircraft card gets an accent glow. All colors from WidgetTheme.

### Dock Panel Backgrounds

Update `render_pane_with_bg()` in `src/dock.rs` to optionally use `GradientPanel` instead of flat `rect_filled`. Subtle top-to-bottom gradient (bg_secondary lightened at top to bg_secondary at bottom) for depth.

## Technical Constraints

- `egui::Shadow` fields are compact integers: `blur: u8`, `spread: u8`, `offset: [i8; 2]`. Max blur is 255px.
- `CornerRadius` uses `u8` fields. Max rounding is 255px per corner.
- Gradient meshes are flat triangle strips with no native corner rounding. Corner masking via border strokes is visually sufficient at small radii (4-8px).
- Vertex color interpolation is in sRGB gamma space, not linear RGB. Subtle difference from CSS gradients but acceptable for UI.
- `Mesh::colored_vertex()` uses `WHITE_UV` (top-left pixel of font texture) so vertex colors pass through unmodified.
