use bevy_egui::egui;

use super::effects::{paint_gradient_rect, paint_multi_gradient_rect, GradientDirection};

enum GradientKind {
    TwoColor {
        start: egui::Color32,
        end: egui::Color32,
        direction: GradientDirection,
    },
    Multi {
        colors: Vec<egui::Color32>,
        direction: GradientDirection,
    },
}

/// A panel that renders a gradient background behind its content.
pub struct GradientPanel {
    gradient: GradientKind,
    corner_radius: u8,
    border: egui::Stroke,
    inner_margin: f32,
}

impl GradientPanel {
    /// Vertical gradient from top color to bottom color.
    pub fn vertical(top: egui::Color32, bottom: egui::Color32) -> Self {
        Self {
            gradient: GradientKind::TwoColor {
                start: top,
                end: bottom,
                direction: GradientDirection::Vertical,
            },
            corner_radius: 0,
            border: egui::Stroke::NONE,
            inner_margin: 0.0,
        }
    }

    /// Horizontal gradient from left color to right color.
    pub fn horizontal(left: egui::Color32, right: egui::Color32) -> Self {
        Self {
            gradient: GradientKind::TwoColor {
                start: left,
                end: right,
                direction: GradientDirection::Horizontal,
            },
            corner_radius: 0,
            border: egui::Stroke::NONE,
            inner_margin: 0.0,
        }
    }

    /// Multi-stop gradient in the given direction.
    pub fn multi(colors: Vec<egui::Color32>, direction: GradientDirection) -> Self {
        Self {
            gradient: GradientKind::Multi { colors, direction },
            corner_radius: 0,
            border: egui::Stroke::NONE,
            inner_margin: 0.0,
        }
    }

    pub fn corner_radius(mut self, radius: u8) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn border(mut self, stroke: egui::Stroke) -> Self {
        self.border = stroke;
        self
    }

    pub fn inner_margin(mut self, margin: f32) -> Self {
        self.inner_margin = margin;
        self
    }

    /// Show the gradient panel with content.
    pub fn show(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) -> egui::InnerResponse<()> {
        let frame = egui::Frame::new()
            .fill(egui::Color32::TRANSPARENT)
            .inner_margin(self.inner_margin)
            .corner_radius(self.corner_radius);

        let mut prepared = frame.begin(ui);
        add_contents(&mut prepared.content_ui);
        let content_rect = prepared.content_ui.min_rect();
        let fill_rect = prepared.frame.fill_rect(content_rect);

        // Paint gradient behind content
        self.paint_gradient(prepared.content_ui.painter(), fill_rect);

        // Corner masking stroke and/or border
        if self.corner_radius > 0 || self.border.width > 0.0 {
            let stroke = if self.border.width > 0.0 {
                self.border
            } else {
                egui::Stroke::NONE
            };
            prepared.content_ui.painter().rect_stroke(
                fill_rect,
                egui::CornerRadius::same(self.corner_radius),
                stroke,
                egui::epaint::StrokeKind::Inside,
            );
        }

        let response = prepared.end(ui);
        egui::InnerResponse::new((), response)
    }

    fn paint_gradient(&self, painter: &egui::Painter, rect: egui::Rect) {
        match &self.gradient {
            GradientKind::TwoColor {
                start,
                end,
                direction,
            } => {
                paint_gradient_rect(
                    painter,
                    rect,
                    *start,
                    *end,
                    match direction {
                        GradientDirection::Vertical => GradientDirection::Vertical,
                        GradientDirection::Horizontal => GradientDirection::Horizontal,
                    },
                );
            }
            GradientKind::Multi { colors, direction } => {
                paint_multi_gradient_rect(
                    painter,
                    rect,
                    colors,
                    match direction {
                        GradientDirection::Vertical => GradientDirection::Vertical,
                        GradientDirection::Horizontal => GradientDirection::Horizontal,
                    },
                );
            }
        }
    }
}
