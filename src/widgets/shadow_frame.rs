use bevy_egui::egui;

use crate::theme::WidgetTheme;

/// Preset shadow intensities.
#[derive(Clone, Copy, Debug)]
pub enum ShadowPreset {
    /// Subtle drop shadow: offset [0,2], blur 8, spread 0.
    Subtle,
    /// Medium drop shadow: offset [0,4], blur 12, spread 0.
    Medium,
    /// Strong drop shadow: offset [2,6], blur 20, spread 2.
    Strong,
}

impl ShadowPreset {
    fn to_shadow(self, color: egui::Color32) -> egui::Shadow {
        match self {
            Self::Subtle => egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color,
            },
            Self::Medium => egui::Shadow {
                offset: [0, 4],
                blur: 12,
                spread: 0,
                color,
            },
            Self::Strong => egui::Shadow {
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
/// Uses egui's native `Shadow` for drop shadows and supports an additional
/// centered glow effect that can be applied simultaneously.
pub struct ShadowFrame {
    shadow: Option<egui::Shadow>,
    glow: Option<egui::Shadow>,
    fill: egui::Color32,
    corner_radius: egui::CornerRadius,
    stroke: egui::Stroke,
    inner_margin: egui::Margin,
}

impl ShadowFrame {
    /// Create a new `ShadowFrame` with defaults derived from the theme.
    ///
    /// Defaults to a `Subtle` shadow preset using the theme's shadow color.
    pub fn new(theme: &WidgetTheme) -> Self {
        Self {
            shadow: Some(ShadowPreset::Subtle.to_shadow(theme.shadow_color)),
            glow: None,
            fill: theme.bg_primary,
            corner_radius: egui::CornerRadius::same(6),
            stroke: egui::Stroke::NONE,
            inner_margin: egui::Margin::same(8),
        }
    }

    /// Apply a shadow preset, keeping the current shadow color.
    pub fn shadow(mut self, preset: ShadowPreset) -> Self {
        let color = self
            .shadow
            .map(|s| s.color)
            .unwrap_or(egui::Color32::from_black_alpha(50));
        self.shadow = Some(preset.to_shadow(color));
        self
    }

    /// Apply a fully custom shadow.
    pub fn custom_shadow(mut self, shadow: egui::Shadow) -> Self {
        self.shadow = Some(shadow);
        self
    }

    /// Add an accent glow (centered shadow with no offset).
    pub fn glow(mut self, color: egui::Color32, blur: u8) -> Self {
        self.glow = Some(egui::Shadow {
            offset: [0, 0],
            blur,
            spread: 0,
            color,
        });
        self
    }

    /// Set the background fill color.
    pub fn fill(mut self, color: egui::Color32) -> Self {
        self.fill = color;
        self
    }

    /// Set the corner radius.
    pub fn corner_radius(mut self, radius: u8) -> Self {
        self.corner_radius = egui::CornerRadius::same(radius);
        self
    }

    /// Set the border stroke.
    pub fn stroke(mut self, stroke: egui::Stroke) -> Self {
        self.stroke = stroke;
        self
    }

    /// Set the inner margin (padding).
    pub fn inner_margin(mut self, margin: f32) -> Self {
        self.inner_margin = egui::Margin::same(margin as i8);
        self
    }

    /// Show the frame with its contents.
    ///
    /// When both shadow and glow are set, the glow is painted behind the frame
    /// using a reserved shape slot, and the drop shadow is applied via the frame.
    pub fn show(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) -> egui::InnerResponse<()> {
        let frame_shadow = self.shadow.unwrap_or(egui::Shadow::NONE);

        let frame = egui::Frame::new()
            .fill(self.fill)
            .corner_radius(self.corner_radius)
            .stroke(self.stroke)
            .inner_margin(self.inner_margin)
            .shadow(frame_shadow);

        if let Some(glow) = self.glow {
            // Reserve a shape slot so the glow paints behind the frame.
            let glow_idx = ui.painter().add(egui::Shape::Noop);

            let response = frame.show(ui, add_contents);

            // Paint glow at the reserved slot using Shadow::as_shape.
            let glow_shape = glow.as_shape(response.response.rect, self.corner_radius);
            ui.painter().set(glow_idx, glow_shape);

            response
        } else {
            frame.show(ui, add_contents)
        }
    }
}
