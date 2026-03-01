# Custom egui Widget Library Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a reusable custom egui widget library (`src/widgets/`) with drop shadows, gradient backgrounds, arc gauges, and themed cards for the aircraft detail display.

**Architecture:** Pure egui Painter API approach — `egui::Shadow` with `RectShape::blur_width` for shadows/glow, `epaint::Mesh` with vertex colors for gradients, `PathShape::line()` for arc gauges. All widgets accept a `WidgetTheme` struct extracted from the existing `AppTheme`.

**Tech Stack:** egui 0.33.3 (via bevy_egui 0.39), epaint 0.33.3, existing Aesthetix theme system

---

### Task 1: Create the widgets module scaffold and WidgetTheme

**Files:**
- Create: `src/widgets/mod.rs`
- Create: `src/widgets/effects.rs`
- Modify: `src/main.rs:6-38` (add `mod widgets;`)
- Modify: `src/theme.rs` (add `WidgetTheme` and `From<&AppTheme>`)

**Step 1: Add `WidgetTheme` to `src/theme.rs`**

After the `AppTheme` impl block (after line 214), add:

```rust
// ── WidgetTheme ─────────────────────────────────────────────────────

/// Lightweight theme colors for custom widgets.
///
/// Extracted from `AppTheme` so widgets don't depend on the full
/// Aesthetix trait or Bevy ECS resources.
pub struct WidgetTheme {
    pub bg_primary: egui::Color32,
    pub bg_secondary: egui::Color32,
    pub accent: egui::Color32,
    pub border: egui::Color32,
    pub text: egui::Color32,
    pub text_dim: egui::Color32,
    pub shadow_color: egui::Color32,
}

impl From<&AppTheme> for WidgetTheme {
    fn from(theme: &AppTheme) -> Self {
        Self {
            bg_primary: theme.inner.bg_primary_color_visuals(),
            bg_secondary: theme.inner.bg_secondary_color_visuals(),
            accent: theme.inner.primary_accent_color_visuals(),
            border: theme.inner.bg_contrast_color_visuals(),
            text: theme.inner.fg_primary_text_color_visuals().unwrap_or(egui::Color32::WHITE),
            text_dim: theme.ext_text_dim,
            shadow_color: egui::Color32::from_black_alpha(50),
        }
    }
}
```

Note: `inner` is private. You'll need to either make the `From` impl inside `theme.rs` (where `inner` is accessible), or add a public method. Since `theme.rs` is in the same crate, implementing `From` directly in `theme.rs` works.

**Step 2: Create `src/widgets/effects.rs`**

Low-level painting helpers:

```rust
use bevy_egui::egui;

/// Direction for gradient rendering.
pub enum GradientDirection {
    Vertical,
    Horizontal,
}

/// Paint a two-color gradient rectangle using a vertex-colored mesh.
pub fn paint_gradient_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    start_color: egui::Color32,
    end_color: egui::Color32,
    direction: GradientDirection,
) {
    let mut mesh = egui::Mesh::default();
    match direction {
        GradientDirection::Vertical => {
            mesh.colored_vertex(rect.left_top(), start_color);
            mesh.colored_vertex(rect.right_top(), start_color);
            mesh.colored_vertex(rect.left_bottom(), end_color);
            mesh.colored_vertex(rect.right_bottom(), end_color);
        }
        GradientDirection::Horizontal => {
            mesh.colored_vertex(rect.left_top(), start_color);
            mesh.colored_vertex(rect.right_top(), end_color);
            mesh.colored_vertex(rect.left_bottom(), start_color);
            mesh.colored_vertex(rect.right_bottom(), end_color);
        }
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// Paint a multi-stop gradient rectangle.
pub fn paint_multi_gradient_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    colors: &[egui::Color32],
    direction: GradientDirection,
) {
    if colors.len() < 2 {
        return;
    }
    let n = colors.len() - 1;
    let mut mesh = egui::Mesh::default();

    for (i, &color) in colors.iter().enumerate() {
        let t = i as f32 / n as f32;
        match direction {
            GradientDirection::Vertical => {
                let y = egui::lerp(rect.top()..=rect.bottom(), t);
                mesh.colored_vertex(egui::pos2(rect.left(), y), color);
                mesh.colored_vertex(egui::pos2(rect.right(), y), color);
            }
            GradientDirection::Horizontal => {
                let x = egui::lerp(rect.left()..=rect.right(), t);
                mesh.colored_vertex(egui::pos2(x, rect.top()), color);
                mesh.colored_vertex(egui::pos2(x, rect.bottom()), color);
            }
        }
        if i < n {
            let idx = (2 * i) as u32;
            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx + 1, idx + 2, idx + 3);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Generate arc points for gauge rendering.
pub fn arc_points(
    center: egui::Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    segments: usize,
) -> Vec<egui::Pos2> {
    (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            let angle = start_angle + t * (end_angle - start_angle);
            egui::pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

/// Paint a stroked arc.
pub fn paint_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    stroke: egui::Stroke,
    segments: usize,
) {
    let points = arc_points(center, radius, start_angle, end_angle, segments);
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(points, stroke)));
}

/// Paint a filled arc band (donut segment) using a triangle mesh.
pub fn paint_thick_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    inner_radius: f32,
    outer_radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: egui::Color32,
    segments: usize,
) {
    let mut mesh = egui::Mesh::default();

    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let angle = start_angle + t * (end_angle - start_angle);
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        mesh.colored_vertex(
            egui::pos2(center.x + inner_radius * cos_a, center.y + inner_radius * sin_a),
            color,
        );
        mesh.colored_vertex(
            egui::pos2(center.x + outer_radius * cos_a, center.y + outer_radius * sin_a),
            color,
        );

        if i < segments {
            let base = (i * 2) as u32;
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 2, base + 3);
        }
    }

    painter.add(egui::Shape::mesh(mesh));
}

/// Linear interpolation between two colors in sRGB space.
pub fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let inv = 1.0 - t;
    egui::Color32::from_rgba_unmultiplied(
        (a.r() as f32 * inv + b.r() as f32 * t) as u8,
        (a.g() as f32 * inv + b.g() as f32 * t) as u8,
        (a.b() as f32 * inv + b.b() as f32 * t) as u8,
        (a.a() as f32 * inv + b.a() as f32 * t) as u8,
    )
}
```

**Step 3: Create `src/widgets/mod.rs`**

```rust
pub mod effects;

pub use effects::{GradientDirection, paint_gradient_rect, paint_multi_gradient_rect, paint_arc, paint_thick_arc, lerp_color, arc_points};
```

**Step 4: Add `mod widgets;` to `src/main.rs`**

Add `mod widgets;` after line 38 (`pub(crate) mod theme;`).

**Step 5: Verify it compiles**

Run: `cargo build 2>&1 | head -30`
Expected: successful build with no errors

**Step 6: Commit**

```
feat: add widgets module scaffold with effects helpers and WidgetTheme
```

---

### Task 2: Implement ShadowFrame

**Files:**
- Create: `src/widgets/shadow_frame.rs`
- Modify: `src/widgets/mod.rs` (add re-export)

**Step 1: Create `src/widgets/shadow_frame.rs`**

```rust
use bevy_egui::egui;
use super::WidgetTheme;

/// Shadow intensity presets.
#[derive(Clone, Copy)]
pub enum ShadowPreset {
    /// Subtle depth hint: offset [0,2], blur 8
    Subtle,
    /// Standard card shadow: offset [0,4], blur 12
    Medium,
    /// Prominent floating panel: offset [2,6], blur 20
    Strong,
}

impl ShadowPreset {
    fn to_shadow(self, color: egui::Color32) -> egui::Shadow {
        match self {
            ShadowPreset::Subtle => egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color,
            },
            ShadowPreset::Medium => egui::Shadow {
                offset: [0, 4],
                blur: 12,
                spread: 0,
                color,
            },
            ShadowPreset::Strong => egui::Shadow {
                offset: [2, 6],
                blur: 20,
                spread: 2,
                color,
            },
        }
    }
}

/// A frame wrapper that adds configurable drop shadow and/or accent glow.
///
/// Built on top of `egui::Frame`, using the native `egui::Shadow` with
/// `blur_width` for GPU-accelerated blur.
pub struct ShadowFrame {
    shadow_color: egui::Color32,
    shadow_preset: Option<ShadowPreset>,
    custom_shadow: Option<egui::Shadow>,
    glow_color: Option<egui::Color32>,
    glow_blur: u8,
    fill: egui::Color32,
    corner_radius: u8,
    stroke: egui::Stroke,
    inner_margin: f32,
}

impl ShadowFrame {
    pub fn new(theme: &WidgetTheme) -> Self {
        Self {
            shadow_color: theme.shadow_color,
            shadow_preset: None,
            custom_shadow: None,
            glow_color: None,
            glow_blur: 16,
            fill: theme.bg_primary,
            corner_radius: 8,
            stroke: egui::Stroke::NONE,
            inner_margin: 12.0,
        }
    }

    /// Apply a shadow preset (Subtle, Medium, Strong).
    pub fn shadow(mut self, preset: ShadowPreset) -> Self {
        self.shadow_preset = Some(preset);
        self
    }

    /// Apply a custom shadow.
    pub fn custom_shadow(mut self, shadow: egui::Shadow) -> Self {
        self.custom_shadow = Some(shadow);
        self
    }

    /// Add an accent-colored glow effect (centered, no offset).
    pub fn glow(mut self, color: egui::Color32, blur: u8) -> Self {
        self.glow_color = Some(color);
        self.glow_blur = blur;
        self
    }

    pub fn fill(mut self, fill: egui::Color32) -> Self {
        self.fill = fill;
        self
    }

    pub fn corner_radius(mut self, radius: u8) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn stroke(mut self, stroke: egui::Stroke) -> Self {
        self.stroke = stroke;
        self
    }

    pub fn inner_margin(mut self, margin: f32) -> Self {
        self.inner_margin = margin;
        self
    }

    /// Show the shadow frame with content.
    pub fn show(self, ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) -> egui::InnerResponse<()> {
        // Determine shadow: custom > preset > none
        let shadow = self.custom_shadow
            .or_else(|| self.shadow_preset.map(|p| p.to_shadow(self.shadow_color)))
            .unwrap_or(egui::Shadow::NONE);

        // If glow is requested, we paint two frames:
        // 1. The glow (as a separate shadow-only frame behind)
        // 2. The actual content frame on top
        if let Some(glow_color) = self.glow_color {
            let glow_shadow = egui::Shadow {
                offset: [0, 0],
                blur: self.glow_blur,
                spread: 2,
                color: glow_color,
            };

            // We need to render the glow behind. Use a nested approach:
            // First render a transparent frame with glow shadow to establish the glow,
            // then the real frame on top.
            let frame = egui::Frame::new()
                .inner_margin(self.inner_margin)
                .fill(self.fill)
                .corner_radius(self.corner_radius)
                .stroke(self.stroke)
                .shadow(glow_shadow);

            // If there's also a drop shadow, we paint it manually before the glow frame
            if shadow != egui::Shadow::NONE {
                // Use begin/end to paint the drop shadow behind everything
                let mut prepared = frame.begin(ui);
                add_contents(&mut prepared.content_ui);
                let rect = prepared.content_ui.min_rect();
                let outer_rect = prepared.frame.fill_rect(rect);

                // Paint drop shadow first (behind glow)
                let shadow_shape = shadow.as_shape(outer_rect, egui::CornerRadius::same(self.corner_radius));
                ui.painter().add(egui::Shape::Rect(shadow_shape));

                prepared.end(ui)
            } else {
                frame.show(ui, add_contents)
            }
        } else {
            egui::Frame::new()
                .inner_margin(self.inner_margin)
                .fill(self.fill)
                .corner_radius(self.corner_radius)
                .stroke(self.stroke)
                .shadow(shadow)
                .show(ui, add_contents)
        }
    }
}
```

**Step 2: Update `src/widgets/mod.rs`**

Add the module and re-exports:

```rust
pub mod effects;
pub mod shadow_frame;

pub use effects::{GradientDirection, paint_gradient_rect, paint_multi_gradient_rect, paint_arc, paint_thick_arc, lerp_color, arc_points};
pub use shadow_frame::{ShadowFrame, ShadowPreset};
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -30`
Expected: successful build

**Step 4: Commit**

```
feat: add ShadowFrame widget with shadow presets and glow support
```

---

### Task 3: Implement GradientPanel

**Files:**
- Create: `src/widgets/gradient_panel.rs`
- Modify: `src/widgets/mod.rs` (add re-export)

**Step 1: Create `src/widgets/gradient_panel.rs`**

```rust
use bevy_egui::egui;
use super::effects::{GradientDirection, paint_gradient_rect, paint_multi_gradient_rect};

/// A panel with a gradient background.
///
/// Uses vertex-colored meshes for GPU-accelerated rendering.
/// Corner rounding is masked with a border stroke since meshes
/// don't support `CornerRadius`.
pub struct GradientPanel {
    colors: Vec<egui::Color32>,
    direction: GradientDirection,
    corner_radius: u8,
    border_stroke: Option<egui::Stroke>,
    inner_margin: f32,
}

impl GradientPanel {
    /// Create a vertical gradient (top color to bottom color).
    pub fn vertical(top: egui::Color32, bottom: egui::Color32) -> Self {
        Self {
            colors: vec![top, bottom],
            direction: GradientDirection::Vertical,
            corner_radius: 0,
            border_stroke: None,
            inner_margin: 8.0,
        }
    }

    /// Create a horizontal gradient (left color to right color).
    pub fn horizontal(left: egui::Color32, right: egui::Color32) -> Self {
        Self {
            colors: vec![left, right],
            direction: GradientDirection::Horizontal,
            corner_radius: 0,
            border_stroke: None,
            inner_margin: 8.0,
        }
    }

    /// Create a multi-stop gradient.
    pub fn multi(colors: Vec<egui::Color32>, direction: GradientDirection) -> Self {
        Self {
            colors,
            direction,
            corner_radius: 0,
            border_stroke: None,
            inner_margin: 8.0,
        }
    }

    pub fn corner_radius(mut self, radius: u8) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn border(mut self, stroke: egui::Stroke) -> Self {
        self.border_stroke = Some(stroke);
        self
    }

    pub fn inner_margin(mut self, margin: f32) -> Self {
        self.inner_margin = margin;
        self
    }

    /// Show the gradient panel with content.
    pub fn show(self, ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) -> egui::InnerResponse<()> {
        let margin = egui::Margin::same(self.inner_margin);
        let frame = egui::Frame::new()
            .inner_margin(margin)
            .fill(egui::Color32::TRANSPARENT)
            .corner_radius(self.corner_radius);

        let mut prepared = frame.begin(ui);
        add_contents(&mut prepared.content_ui);
        let content_rect = prepared.content_ui.min_rect();
        let fill_rect = prepared.frame.fill_rect(content_rect);

        // Paint gradient behind content
        if self.colors.len() == 2 {
            paint_gradient_rect(
                prepared.content_ui.painter(),
                fill_rect,
                self.colors[0],
                self.colors[1],
                self.direction,
            );
        } else if self.colors.len() > 2 {
            paint_multi_gradient_rect(
                prepared.content_ui.painter(),
                fill_rect,
                &self.colors,
                self.direction,
            );
        }

        // Corner masking stroke
        if self.corner_radius > 0 || self.border_stroke.is_some() {
            let stroke = self.border_stroke.unwrap_or(egui::Stroke::NONE);
            if stroke.width > 0.0 || self.corner_radius > 0 {
                prepared.content_ui.painter().rect_stroke(
                    fill_rect,
                    egui::CornerRadius::same(self.corner_radius),
                    stroke,
                    egui::epaint::StrokeKind::Inside,
                );
            }
        }

        prepared.end(ui)
    }
}
```

**Step 2: Update `src/widgets/mod.rs`**

Add `pub mod gradient_panel;` and `pub use gradient_panel::GradientPanel;`

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -30`

**Step 4: Commit**

```
feat: add GradientPanel widget with vertical/horizontal/multi-stop gradients
```

---

### Task 4: Implement Card widget

**Files:**
- Create: `src/widgets/card.rs`
- Modify: `src/widgets/mod.rs` (add re-export)

**Step 1: Create `src/widgets/card.rs`**

```rust
use bevy_egui::egui;
use super::effects::{GradientDirection, paint_gradient_rect};
use super::shadow_frame::{ShadowFrame, ShadowPreset};
use super::WidgetTheme;

/// A card container with optional gradient header, shadow, and themed styling.
pub struct Card<'a> {
    theme: &'a WidgetTheme,
    header_text: Option<String>,
    header_gradient: Option<(egui::Color32, egui::Color32)>,
    shadow_preset: Option<ShadowPreset>,
    corner_radius: u8,
    glow_color: Option<egui::Color32>,
}

impl<'a> Card<'a> {
    pub fn new(theme: &'a WidgetTheme) -> Self {
        Self {
            theme,
            header_text: None,
            header_gradient: None,
            shadow_preset: Some(ShadowPreset::Subtle),
            corner_radius: 8,
            glow_color: None,
        }
    }

    /// Set the card header text.
    pub fn header(mut self, text: impl Into<String>) -> Self {
        self.header_text = Some(text.into());
        self
    }

    /// Set a gradient for the header bar.
    pub fn gradient_header(mut self, left: egui::Color32, right: egui::Color32) -> Self {
        self.header_gradient = Some((left, right));
        self
    }

    /// Set the shadow preset.
    pub fn shadow(mut self, preset: ShadowPreset) -> Self {
        self.shadow_preset = Some(preset);
        self
    }

    /// Disable the shadow.
    pub fn no_shadow(mut self) -> Self {
        self.shadow_preset = None;
        self
    }

    pub fn corner_radius(mut self, radius: u8) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Add an accent glow effect.
    pub fn glow(mut self, color: egui::Color32) -> Self {
        self.glow_color = Some(color);
        self
    }

    /// Show the card with content.
    pub fn show(self, ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) -> egui::InnerResponse<()> {
        let mut frame = ShadowFrame::new(self.theme)
            .corner_radius(self.corner_radius)
            .stroke(egui::Stroke::new(1.0, self.theme.border))
            .inner_margin(0.0); // We handle margins inside for header separation

        if let Some(preset) = self.shadow_preset {
            frame = frame.shadow(preset);
        }

        if let Some(glow_color) = self.glow_color {
            frame = frame.glow(glow_color, 16);
        }

        frame.show(ui, |ui| {
            // Header
            if let Some(ref header_text) = self.header_text {
                let header_rect = ui.allocate_space(egui::vec2(ui.available_width(), 28.0)).1;

                // Gradient or solid header background
                if let Some((left, right)) = self.header_gradient {
                    paint_gradient_rect(
                        ui.painter(),
                        header_rect,
                        left,
                        right,
                        GradientDirection::Horizontal,
                    );
                } else {
                    ui.painter().rect_filled(
                        header_rect,
                        egui::CornerRadius::ZERO,
                        self.theme.bg_secondary,
                    );
                }

                // Header text
                ui.painter().text(
                    header_rect.left_center() + egui::vec2(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    header_text,
                    egui::FontId::proportional(12.0),
                    self.theme.text,
                );

                // Separator line under header
                ui.painter().hline(
                    header_rect.x_range(),
                    header_rect.bottom(),
                    egui::Stroke::new(1.0, self.theme.border),
                );
            }

            // Body content with margin
            egui::Frame::new()
                .inner_margin(10.0)
                .show(ui, |ui| {
                    add_contents(ui);
                });
        })
    }
}
```

**Step 2: Update `src/widgets/mod.rs`**

Add `pub mod card;` and `pub use card::Card;`

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -30`

**Step 4: Commit**

```
feat: add Card widget with gradient header and shadow/glow support
```

---

### Task 5: Implement ArcGauge widget

**Files:**
- Create: `src/widgets/gauge.rs`
- Modify: `src/widgets/mod.rs` (add re-export)

**Step 1: Create `src/widgets/gauge.rs`**

```rust
use bevy_egui::egui;
use super::effects::{arc_points, paint_arc};
use super::WidgetTheme;

/// A circular arc gauge widget.
///
/// Renders a curved track with a filled value arc, tick marks,
/// center label, and value text. Implements `egui::Widget`.
pub struct ArcGauge<'a> {
    /// Value in 0.0..=1.0 range
    value: f32,
    /// Label shown at center (e.g., "ALT")
    label: Option<&'a str>,
    /// Formatted value text (e.g., "FL350")
    value_text: Option<&'a str>,
    /// Widget size in pixels (width = height)
    size: f32,
    /// Arc sweep angle in degrees (default 270)
    sweep_degrees: f32,
    /// Track (background) color
    track_color: egui::Color32,
    /// Fill (value) color
    fill_color: egui::Color32,
    /// Label text color
    text_color: egui::Color32,
    /// Value text color
    value_color: egui::Color32,
    /// Number of tick marks
    tick_count: usize,
    /// Track stroke width
    track_width: f32,
    /// Value stroke width
    fill_width: f32,
}

impl<'a> ArcGauge<'a> {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            label: None,
            value_text: None,
            size: 100.0,
            sweep_degrees: 270.0,
            track_color: egui::Color32::from_gray(60),
            fill_color: egui::Color32::from_rgb(0, 150, 255),
            text_color: egui::Color32::from_gray(180),
            value_color: egui::Color32::WHITE,
            tick_count: 10,
            track_width: 6.0,
            fill_width: 6.0,
        }
    }

    /// Construct from a WidgetTheme with sensible defaults.
    pub fn themed(value: f32, theme: &WidgetTheme) -> Self {
        Self::new(value)
            .track_color(theme.border)
            .fill_color(theme.accent)
            .text_color(theme.text_dim)
            .value_color(theme.text)
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn value_text(mut self, text: &'a str) -> Self {
        self.value_text = Some(text);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn sweep(mut self, degrees: f32) -> Self {
        self.sweep_degrees = degrees;
        self
    }

    pub fn track_color(mut self, color: egui::Color32) -> Self {
        self.track_color = color;
        self
    }

    pub fn fill_color(mut self, color: egui::Color32) -> Self {
        self.fill_color = color;
        self
    }

    pub fn text_color(mut self, color: egui::Color32) -> Self {
        self.text_color = color;
        self
    }

    pub fn value_color(mut self, color: egui::Color32) -> Self {
        self.value_color = color;
        self
    }

    pub fn tick_count(mut self, count: usize) -> Self {
        self.tick_count = count;
        self
    }

    pub fn track_width(mut self, width: f32) -> Self {
        self.track_width = width;
        self
    }

    pub fn fill_width(mut self, width: f32) -> Self {
        self.fill_width = width;
        self
    }
}

impl egui::Widget for ArcGauge<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = egui::vec2(self.size, self.size);
        let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());

        let center = response.rect.center();
        let radius = self.size / 2.0 - self.track_width - 4.0;
        let segments = 64;

        // Arc angles: sweep centered at bottom
        let half_gap = (360.0 - self.sweep_degrees) / 2.0;
        let start_angle = (90.0 + half_gap).to_radians();
        let end_angle = (90.0 + 360.0 - half_gap).to_radians();

        // Background track
        paint_arc(
            &painter,
            center,
            radius,
            start_angle,
            end_angle,
            egui::Stroke::new(self.track_width, self.track_color),
            segments,
        );

        // Value arc
        if self.value > 0.001 {
            let value_angle = start_angle + (end_angle - start_angle) * self.value;
            let value_segments = ((segments as f32) * self.value).max(2.0) as usize;
            paint_arc(
                &painter,
                center,
                radius,
                start_angle,
                value_angle,
                egui::Stroke::new(self.fill_width, self.fill_color),
                value_segments,
            );
        }

        // Tick marks
        if self.tick_count > 0 {
            for i in 0..=self.tick_count {
                let t = i as f32 / self.tick_count as f32;
                let angle = start_angle + t * (end_angle - start_angle);
                let cos_a = angle.cos();
                let sin_a = angle.sin();
                let inner = radius - self.track_width / 2.0 - 3.0;
                let outer = radius - self.track_width / 2.0 - 8.0;
                let p1 = egui::pos2(center.x + inner * cos_a, center.y + inner * sin_a);
                let p2 = egui::pos2(center.x + outer * cos_a, center.y + outer * sin_a);
                painter.line_segment(
                    [p1, p2],
                    egui::Stroke::new(1.0, self.track_color),
                );
            }
        }

        // Center label (e.g., "ALT")
        if let Some(label) = self.label {
            painter.text(
                center + egui::vec2(0.0, -8.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(self.size * 0.12),
                self.text_color,
            );
        }

        // Value text (e.g., "FL350")
        if let Some(value_text) = self.value_text {
            painter.text(
                center + egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_CENTER,
                value_text,
                egui::FontId::proportional(self.size * 0.16),
                self.value_color,
            );
        }

        response
    }
}
```

**Step 2: Update `src/widgets/mod.rs`**

Add `pub mod gauge;` and `pub use gauge::ArcGauge;`

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -30`

**Step 4: Commit**

```
feat: add ArcGauge widget with track, value arc, ticks, and labels
```

---

### Task 6: Implement DataStrip widget

**Files:**
- Create: `src/widgets/data_strip.rs`
- Modify: `src/widgets/mod.rs` (add re-export)

**Step 1: Create `src/widgets/data_strip.rs`**

```rust
use bevy_egui::egui;
use super::WidgetTheme;

/// A horizontal data ribbon with an optional colored accent border on the left.
pub struct DataStrip<'a> {
    theme: &'a WidgetTheme,
    accent_color: Option<egui::Color32>,
    accent_width: f32,
    glow_blur: Option<u8>,
    fill: Option<egui::Color32>,
    inner_margin: f32,
}

impl<'a> DataStrip<'a> {
    pub fn new(theme: &'a WidgetTheme) -> Self {
        Self {
            theme,
            accent_color: None,
            accent_width: 3.0,
            glow_blur: None,
            fill: None,
            inner_margin: 6.0,
        }
    }

    /// Add a colored accent bar on the left edge.
    pub fn accent_left(mut self, color: egui::Color32, width: f32) -> Self {
        self.accent_color = Some(color);
        self.accent_width = width;
        self
    }

    /// Add a glow effect around the accent border.
    pub fn glow(mut self, blur: u8) -> Self {
        self.glow_blur = Some(blur);
        self
    }

    pub fn fill(mut self, fill: egui::Color32) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn inner_margin(mut self, margin: f32) -> Self {
        self.inner_margin = margin;
        self
    }

    /// Show the data strip with content.
    pub fn show(self, ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) -> egui::InnerResponse<()> {
        let fill = self.fill.unwrap_or(self.theme.bg_secondary);

        let frame = egui::Frame::new()
            .inner_margin(egui::Margin {
                left: self.inner_margin + if self.accent_color.is_some() { self.accent_width + 2.0 } else { 0.0 },
                right: self.inner_margin,
                top: self.inner_margin / 2.0,
                bottom: self.inner_margin / 2.0,
            })
            .fill(fill)
            .corner_radius(4);

        let mut prepared = frame.begin(ui);
        add_contents(&mut prepared.content_ui);
        let content_rect = prepared.content_ui.min_rect();
        let fill_rect = prepared.frame.fill_rect(content_rect);

        // Paint accent bar on left edge
        if let Some(accent) = self.accent_color {
            let accent_rect = egui::Rect::from_min_size(
                fill_rect.left_top(),
                egui::vec2(self.accent_width, fill_rect.height()),
            );

            // Optional glow behind accent bar
            if let Some(blur) = self.glow_blur {
                let glow_shape = egui::epaint::RectShape::filled(
                    accent_rect.expand(2.0),
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 60),
                ).with_blur_width(blur as f32);
                prepared.content_ui.painter().add(egui::Shape::Rect(glow_shape));
            }

            prepared.content_ui.painter().rect_filled(
                accent_rect,
                egui::CornerRadius::ZERO,
                accent,
            );
        }

        prepared.end(ui)
    }
}
```

**Step 2: Update `src/widgets/mod.rs`**

Add `pub mod data_strip;` and `pub use data_strip::DataStrip;`

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -30`

**Step 4: Commit**

```
feat: add DataStrip widget with accent border and glow support
```

---

### Task 7: Update final `src/widgets/mod.rs` with all re-exports

**Files:**
- Modify: `src/widgets/mod.rs`

**Step 1: Ensure `src/widgets/mod.rs` has all modules and re-exports**

```rust
mod effects;
pub mod shadow_frame;
pub mod gradient_panel;
pub mod card;
pub mod gauge;
pub mod data_strip;

// Re-export WidgetTheme from theme module
pub use crate::theme::WidgetTheme;

// Re-export low-level effects
pub use effects::{
    GradientDirection,
    paint_gradient_rect,
    paint_multi_gradient_rect,
    paint_arc,
    paint_thick_arc,
    lerp_color,
    arc_points,
};

// Re-export widgets
pub use shadow_frame::{ShadowFrame, ShadowPreset};
pub use gradient_panel::GradientPanel;
pub use card::Card;
pub use gauge::ArcGauge;
pub use data_strip::DataStrip;
```

**Step 2: Verify the full build**

Run: `cargo build 2>&1 | head -30`

**Step 3: Commit**

```
chore: finalize widgets module re-exports
```

---

### Task 8: Integrate gradient backgrounds into dock panels

**Files:**
- Modify: `src/dock.rs:247-248` (`render_pane_with_bg`)

**Step 1: Update `render_pane_with_bg` to use a subtle gradient**

In `src/dock.rs`, modify the `render_pane_with_bg` function (around line 247):

```rust
/// Paint opaque background with subtle gradient and wrap content in a vertical scroll area.
fn render_pane_with_bg(bg: egui::Color32, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    // Subtle vertical gradient: slightly lighter at top, base color at bottom
    let top_color = crate::widgets::lerp_color(bg, egui::Color32::WHITE, 0.04);
    crate::widgets::paint_gradient_rect(
        ui.painter(),
        ui.max_rect(),
        top_color,
        bg,
        crate::widgets::GradientDirection::Vertical,
    );
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(4.0);
            content(ui);
        });
}
```

**Step 2: Verify it compiles and run visually**

Run: `cargo build 2>&1 | head -30`
Then: `cargo run` and check that dock panels have a subtle gradient.

**Step 3: Commit**

```
feat: add subtle gradient backgrounds to dock panels
```

---

### Task 9: Integrate custom widgets into the aircraft detail display

**Files:**
- Modify: `src/aircraft/list_panel.rs:930-1062` (`render_inline_detail`)

**Step 1: Update `render_inline_detail` to use Card and DataStrip widgets**

Replace the existing `render_inline_detail` function body with custom widget usage. The function signature stays the same. Import the widgets at the top of the file:

```rust
use crate::widgets::{Card, DataStrip, ArcGauge, WidgetTheme};
use crate::theme::AppTheme;
```

Then update `render_inline_detail` to use the new widgets:

```rust
fn render_inline_detail(
    ui: &mut egui::Ui,
    selected_icao: &str,
    expand_t: f32,
    follow_state: &mut CameraFollowState,
    app_config: &crate::config::AppConfig,
    clock: &SessionClock,
    aircraft_query: &Query<(&crate::Aircraft, &TrailHistory, Option<&AircraftTypeInfo>)>,
) {
    let Some((aircraft, trail, type_info)) = aircraft_query.iter().find(|(a, _, _)| a.icao == selected_icao) else {
        return;
    };

    // Build a WidgetTheme from hardcoded colors matching the current panel style.
    // TODO: Pass AppTheme through once the function signature can be extended.
    let wt = WidgetTheme {
        bg_primary: egui::Color32::from_gray(30),
        bg_secondary: egui::Color32::from_gray(38),
        accent: egui::Color32::from_rgb(100, 200, 255),
        border: egui::Color32::from_gray(55),
        text: egui::Color32::from_rgb(220, 220, 220),
        text_dim: egui::Color32::from_rgb(150, 150, 150),
        shadow_color: egui::Color32::from_black_alpha(50),
    };

    let distance_nm = haversine_distance_nm(
        app_config.map.default_latitude,
        app_config.map.default_longitude,
        aircraft.latitude,
        aircraft.longitude,
    );

    let oldest_point_age = trail.points.front().map(|p| clock.age_secs(p.timestamp) as u64);

    // Measure content height for animation clipping
    let detail_id = ui.id().with(selected_icao).with("detail_content");
    let full_height = ui.ctx().memory(|mem| {
        mem.data.get_temp::<f32>(detail_id).unwrap_or(200.0)
    });
    let visible_height = full_height * expand_t;

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(2.0);

    let response = ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), visible_height),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.set_clip_rect(ui.max_rect());

            // Position data strip
            DataStrip::new(&wt)
                .accent_left(wt.accent, 3.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Pos").color(wt.text_dim).size(10.0));
                        ui.label(
                            egui::RichText::new(format!("{:.4}, {:.4}", aircraft.latitude, aircraft.longitude))
                                .color(wt.text).size(10.0).monospace(),
                        );
                    });
                });

            ui.add_space(2.0);

            // Key metrics as data strips
            let mut pairs: Vec<(&str, String, egui::Color32)> = Vec::new();
            pairs.push(("Dist", format!("{:.1}nm", distance_nm), wt.accent));

            if let Some(ti) = type_info {
                if let Some(ref reg) = ti.registration {
                    pairs.push(("Reg", reg.clone(), wt.text));
                }
                if let Some(ref tc) = ti.type_code {
                    pairs.push(("Type", tc.clone(), wt.text));
                }
                if let Some(ref op) = ti.operator {
                    pairs.push(("Oper", op.clone(), wt.text));
                }
            }

            pairs.push(("Trk", format!("{}", trail.points.len()), wt.text));

            let dur_text = oldest_point_age
                .map(|secs| format!("{}:{:02}", secs / 60, secs % 60))
                .unwrap_or_else(|| "---".to_string());
            pairs.push(("Dur", dur_text, wt.text));

            // Render pairs as data strips, 2 per row
            for chunk in pairs.chunks(2) {
                DataStrip::new(&wt)
                    .accent_left(wt.border, 2.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(chunk[0].0).color(wt.text_dim).size(10.0));
                            ui.label(egui::RichText::new(&chunk[0].1).color(chunk[0].2).size(10.0).monospace());
                            if let Some(pair) = chunk.get(1) {
                                ui.add_space(12.0);
                                ui.label(egui::RichText::new(pair.0).color(wt.text_dim).size(10.0));
                                ui.label(egui::RichText::new(&pair.1).color(pair.2).size(10.0).monospace());
                            }
                        });
                    });
                ui.add_space(1.0);
            }

            ui.add_space(2.0);

            // Gauges row for key flight metrics
            ui.horizontal(|ui| {
                if let Some(alt) = aircraft.altitude {
                    let alt_norm = (alt as f32 / 45000.0).clamp(0.0, 1.0);
                    ui.add(ArcGauge::themed(alt_norm, &wt)
                        .size(60.0)
                        .label("ALT")
                        .value_text(&format!("{}", alt))
                        .tick_count(5)
                        .track_width(4.0)
                        .fill_width(4.0));
                }

                if let Some(speed) = aircraft.velocity {
                    let spd_norm = (speed as f32 / 600.0).clamp(0.0, 1.0);
                    ui.add(ArcGauge::themed(spd_norm, &wt)
                        .size(60.0)
                        .label("SPD")
                        .value_text(&format!("{:.0}", speed))
                        .fill_color(egui::Color32::from_rgb(100, 220, 150))
                        .tick_count(5)
                        .track_width(4.0)
                        .fill_width(4.0));
                }

                if let Some(hdg) = aircraft.heading {
                    let hdg_norm = hdg / 360.0;
                    ui.add(ArcGauge::themed(hdg_norm, &wt)
                        .size(60.0)
                        .label("HDG")
                        .value_text(&format!("{:.0}", hdg))
                        .fill_color(egui::Color32::from_rgb(220, 180, 100))
                        .sweep(360.0)
                        .tick_count(8)
                        .track_width(4.0)
                        .fill_width(4.0));
                }
            });

            ui.add_space(2.0);

            // Follow/Unfollow button
            ui.horizontal(|ui| {
                let is_following = follow_state.following_icao.as_deref() == Some(selected_icao);
                let follow_text = if is_following { "Unfollow" } else { "Follow" };
                let follow_color = if is_following {
                    egui::Color32::from_rgb(255, 100, 100)
                } else {
                    wt.accent
                };
                if ui.add(egui::Button::new(
                    egui::RichText::new(follow_text)
                        .color(follow_color)
                        .size(10.0)
                ).small()).clicked() {
                    if is_following {
                        follow_state.following_icao = None;
                    } else {
                        follow_state.following_icao = Some(selected_icao.to_string());
                    }
                }
            });
        },
    );

    let actual_height = response.response.rect.height().max(10.0);
    if expand_t >= 1.0 {
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(detail_id, actual_height);
        });
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -30`

**Step 3: Run and visually verify**

Run: `cargo run`
- Select an aircraft from the list
- Verify the detail section shows data strips with accent borders and arc gauges
- Verify the animation still works correctly

**Step 4: Commit**

```
feat: integrate custom widgets into aircraft detail display
```

---

### Task 10: Visual polish and final testing

**Files:**
- Potentially fine-tune: `src/widgets/shadow_frame.rs`, `src/widgets/gauge.rs`, `src/widgets/data_strip.rs`

**Step 1: Run the application and test all themes**

Run: `cargo run`
- Switch between all 10 themes in settings
- Verify dock panel gradients look correct in both dark and light themes
- Verify aircraft detail widgets render correctly
- Check that shadow/glow effects don't bleed outside panel boundaries

**Step 2: Test edge cases**

- No aircraft selected (empty detail section)
- Aircraft with missing data (no altitude, no speed, no heading)
- Rapidly selecting/deselecting aircraft (animation)
- Window resize with gauges

**Step 3: Adjust colors/sizes if needed**

Fine-tune any values that don't look right visually. Commit adjustments.

**Step 4: Final commit**

```
polish: tune widget colors and sizes for visual consistency
```
