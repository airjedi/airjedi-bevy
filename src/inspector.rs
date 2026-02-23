/// ECS Inspector window using bevy-inspector-egui.
///
/// Provides a floating egui window with live, editable views of
/// entities, resources, and assets. Rendered by an exclusive system
/// that requires &mut World access.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContext};
use bevy_inspector_egui::bevy_inspector;

use crate::debug_panel::DebugPanelState;
use crate::map::{MapState, ZoomState};
use crate::ui_panels::{PanelId, UiPanelManager};
use crate::view3d::View3DState;

/// Resource controlling inspector window visibility.
#[derive(Resource, Default)]
pub struct InspectorState {
    pub open: bool,
}

/// Exclusive system that renders the inspector window.
///
/// Must be exclusive because `bevy_inspector` functions require `&mut World`.
/// Runs in `Update` schedule, not `EguiPrimaryContextPass`.
pub fn render_inspector_window(world: &mut World) {
    // Check if inspector should be shown
    let open = world
        .get_resource::<UiPanelManager>()
        .is_some_and(|panels| panels.is_open(PanelId::Inspector));

    if !open {
        return;
    }

    // Clone the egui context so we can release the world borrow
    let mut egui_context = world
        .query_filtered::<&mut EguiContext, With<bevy_egui::PrimaryEguiContext>>()
        .single(world)
        .expect("EguiContext not found")
        .clone();

    let ctx = egui_context.get_mut();

    egui::Window::new("Inspector")
        .default_size([400.0, 500.0])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui: &mut egui::Ui| {
            egui::ScrollArea::both().show(ui, |ui: &mut egui::Ui| {
                // Section 1: Curated app resources (open by default)
                egui::CollapsingHeader::new("App Resources")
                    .default_open(true)
                    .show(ui, |ui: &mut egui::Ui| {
                        ui.label("MapState");
                        bevy_inspector::ui_for_resource::<MapState>(world, ui);
                        ui.separator();

                        ui.label("ZoomState");
                        bevy_inspector::ui_for_resource::<ZoomState>(world, ui);
                        ui.separator();

                        ui.label("View3DState");
                        bevy_inspector::ui_for_resource::<View3DState>(world, ui);
                        ui.separator();

                        ui.label("DebugPanelState");
                        bevy_inspector::ui_for_resource::<DebugPanelState>(world, ui);
                    });

                ui.separator();

                // Section 2: All entities
                egui::CollapsingHeader::new("Entities")
                    .default_open(false)
                    .show(ui, |ui: &mut egui::Ui| {
                        bevy_inspector::ui_for_entities(world, ui);
                    });

                ui.separator();

                // Section 3: All resources
                egui::CollapsingHeader::new("Resources")
                    .default_open(false)
                    .show(ui, |ui: &mut egui::Ui| {
                        bevy_inspector::ui_for_resources(world, ui);
                    });

                ui.separator();

                // Section 4: All assets
                egui::CollapsingHeader::new("Assets")
                    .default_open(false)
                    .show(ui, |ui: &mut egui::Ui| {
                        bevy_inspector::ui_for_all_assets(world, ui);
                    });
            });
        });
}
