# Statusbar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a bottom statusbar showing connection status, aircraft count, message rate, FPS, recording state, and map position/zoom.

**Architecture:** An egui `TopBottomPanel::bottom()` rendered before the CentralPanel in the EguiPrimaryContextPass schedule. A new `statusbar.rs` module with a `StatusBarState` resource (FPS smoothing) and a `render_statusbar` system. The existing recording indicator overlay and map attribution overlay are removed and consolidated into the statusbar.

**Tech Stack:** Bevy 0.18, bevy_egui 0.39, egui

---

### Task 1: Create `src/statusbar.rs` with StatusBarState and render_statusbar

**Files:**
- Create: `src/statusbar.rs`

**Step 1: Create the statusbar module**

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::adsb::AdsbAircraftData;
use crate::aircraft::stats_panel::StatsPanelState;
use crate::recording::RecordingState;
use crate::theme::{AppTheme, to_egui_color32, to_egui_color32_alpha};
use crate::MapState;

/// FPS smoothing state using exponential moving average.
#[derive(Resource)]
pub struct StatusBarState {
    /// Smoothed FPS value
    pub fps: f32,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self { fps: 0.0 }
    }
}

/// Height of the statusbar in pixels.
const STATUSBAR_HEIGHT: f32 = 22.0;
/// Font size for all statusbar text.
const FONT_SIZE: f32 = 11.0;
/// EMA smoothing factor for FPS (lower = smoother, 0.05 = ~1s window at 60fps).
const FPS_SMOOTHING: f32 = 0.05;

/// Render the bottom statusbar as an egui BottomPanel.
///
/// Must run before the CentralPanel (dock tree) in EguiPrimaryContextPass.
pub fn render_statusbar(
    mut contexts: EguiContexts,
    theme: Res<AppTheme>,
    adsb_data: Option<Res<AdsbAircraftData>>,
    stats: Res<StatsPanelState>,
    recording: Res<RecordingState>,
    map_state: Res<MapState>,
    time: Res<Time>,
    mut state: ResMut<StatusBarState>,
) {
    // Update FPS with exponential moving average
    let dt = time.delta_secs();
    if dt > 0.0 {
        let instant_fps = 1.0 / dt;
        if state.fps == 0.0 {
            state.fps = instant_fps;
        } else {
            state.fps += FPS_SMOOTHING * (instant_fps - state.fps);
        }
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let panel_bg = to_egui_color32(theme.bg_secondary());
    let border_color = to_egui_color32(theme.bg_contrast());
    let dim = to_egui_color32(theme.text_dim());
    let primary = to_egui_color32(theme.text_primary());

    let frame = egui::Frame::default()
        .fill(panel_bg)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::symmetric(8, 2));

    egui::TopBottomPanel::bottom("statusbar")
        .exact_height(STATUSBAR_HEIGHT)
        .frame(frame)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // -- Connection status --
                render_connection_section(ui, &adsb_data, &theme);

                separator(ui, dim);

                // -- Aircraft count --
                let count = adsb_data
                    .as_ref()
                    .and_then(|d| d.try_aircraft_count())
                    .unwrap_or(0);
                ui.label(egui::RichText::new(format!("{} aircraft", count)).size(FONT_SIZE).color(primary));

                separator(ui, dim);

                // -- Message rate --
                ui.label(
                    egui::RichText::new(format!("{:.0} msg/s", stats.message_rate))
                        .size(FONT_SIZE)
                        .color(primary),
                );

                separator(ui, dim);

                // -- FPS --
                ui.label(
                    egui::RichText::new(format!("{:.0} FPS", state.fps))
                        .size(FONT_SIZE)
                        .color(primary),
                );

                // -- Recording indicator (only when active) --
                if recording.is_recording {
                    separator(ui, dim);
                    let time_val = ui.input(|i| i.time);
                    let alpha = if (time_val * 2.0) as i32 % 2 == 0 { 255 } else { 100 };
                    let rec_color = egui::Color32::from_rgba_unmultiplied(255, 0, 0, alpha);
                    ui.label(
                        egui::RichText::new(format!("REC {}s", recording.duration_secs()))
                            .size(FONT_SIZE)
                            .color(rec_color)
                            .strong(),
                    );
                }

                // -- Right-aligned: map position + attribution --
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    // Attribution (rightmost)
                    ui.label(
                        egui::RichText::new("\u{00A9} OSM, CartoDB")
                            .size(FONT_SIZE)
                            .color(dim),
                    );

                    separator(ui, dim);

                    // Map position + zoom
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.4}, {:.4}  Z{}",
                            map_state.latitude,
                            map_state.longitude,
                            map_state.zoom_level.to_u8(),
                        ))
                        .size(FONT_SIZE)
                        .color(primary),
                    );
                });
            });
        });
}

/// Render the connection status dot and label.
fn render_connection_section(
    ui: &mut egui::Ui,
    adsb_data: &Option<Res<AdsbAircraftData>>,
    theme: &AppTheme,
) {
    let Some(data) = adsb_data else {
        ui.label(
            egui::RichText::new("\u{25CF} No client")
                .size(FONT_SIZE)
                .color(to_egui_color32(theme.text_dim())),
        );
        return;
    };

    let connection_state = data.get_connection_state();
    use adsb_client::ConnectionState;
    let (color, label) = match connection_state {
        ConnectionState::Connected => (to_egui_color32(theme.text_success()), "Connected"),
        ConnectionState::Connecting => (to_egui_color32(theme.text_warn()), "Connecting"),
        ConnectionState::Disconnected => (to_egui_color32(theme.text_error()), "Disconnected"),
        ConnectionState::Error(_) => (to_egui_color32(theme.text_error()), "Error"),
    };

    ui.label(egui::RichText::new("\u{25CF}").size(FONT_SIZE).color(color));
    ui.label(egui::RichText::new(label).size(FONT_SIZE).color(color));
}

/// Draw a dim vertical separator between statusbar sections.
fn separator(ui: &mut egui::Ui, color: egui::Color32) {
    ui.label(egui::RichText::new("|").size(FONT_SIZE).color(color));
}
```

**Step 2: Verify it compiles in isolation (syntax check)**

This file won't compile alone -- it will be checked in Task 2 when wired in.

---

### Task 2: Wire statusbar into main.rs

**Files:**
- Modify: `src/main.rs:28` (add mod declaration)
- Modify: `src/main.rs:304-307` (add resource init)
- Modify: `src/main.rs:338-343` (add system to EguiPrimaryContextPass, remove render_map_attribution)

**Step 1: Add the module declaration**

In `src/main.rs`, after `mod dock;` (line 28), add:

```rust
mod statusbar;
```

**Step 2: Register the StatusBarState resource**

In `src/main.rs`, after `.init_resource::<dock::DockTreeState>()` (line 307), add:

```rust
.init_resource::<statusbar::StatusBarState>()
```

**Step 3: Add render_statusbar to EguiPrimaryContextPass and remove render_map_attribution**

Replace the `EguiPrimaryContextPass` system block (lines 338-343):

```rust
.add_systems(bevy_egui::EguiPrimaryContextPass, (
    theme::apply_egui_theme,
    toolbar::render_toolbar,
    statusbar::render_statusbar.after(toolbar::render_toolbar),
    dock::render_dock_tree.after(statusbar::render_statusbar),
))
```

This removes `toolbar::render_map_attribution` and adds `statusbar::render_statusbar` with correct ordering (toolbar -> statusbar -> dock).

**Step 4: Build and verify**

Run: `cargo build 2>&1 | head -30`
Expected: Successful compilation (or only unrelated warnings)

**Step 5: Commit**

```bash
git add src/statusbar.rs src/main.rs
git commit -m "Add bottom statusbar with connection, aircraft, msg rate, FPS, position"
```

---

### Task 3: Remove the floating recording indicator overlay

**Files:**
- Modify: `src/recording/recorder.rs:224-253` (remove `render_recording_indicator` function)
- Modify: `src/recording/mod.rs:21` (remove system registration)

**Step 1: Remove render_recording_indicator from recorder.rs**

Delete the entire `render_recording_indicator` function (lines 224-253 in `src/recording/recorder.rs`), including its doc comment.

**Step 2: Remove from plugin registration**

In `src/recording/mod.rs`, remove line 21:
```rust
            .add_systems(EguiPrimaryContextPass, render_recording_indicator);
```

Also remove the `use bevy_egui::EguiPrimaryContextPass;` import on line 8 if it becomes unused.

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -30`
Expected: Successful compilation. No more floating REC overlay -- it now lives in the statusbar.

**Step 4: Commit**

```bash
git add src/recording/recorder.rs src/recording/mod.rs
git commit -m "Remove floating recording indicator overlay, now in statusbar"
```

---

### Task 4: Remove the floating map attribution overlay

**Files:**
- Modify: `src/toolbar.rs:191-215` (remove `render_map_attribution` function)

**Step 1: Remove render_map_attribution from toolbar.rs**

Delete the entire `render_map_attribution` function (lines 191-215 in `src/toolbar.rs`), including its doc comment.

**Step 2: Clean up unused imports in toolbar.rs**

After removing `render_map_attribution`, check if any imports are now unused (e.g., the `to_egui_color32_alpha` import may still be used by `render_toolbar`). Remove only truly unused ones.

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -30`
Expected: Successful compilation. Attribution text now only appears in the statusbar.

**Step 4: Commit**

```bash
git add src/toolbar.rs
git commit -m "Remove floating map attribution overlay, now in statusbar"
```

---

### Task 5: Visual verification

**Step 1: Run the application**

Run: `cargo run`

**Step 2: Verify visually**

Check for:
- Statusbar visible at bottom of window, below dock panels
- Connection dot with status text (green if connected)
- Aircraft count updating
- Message rate showing
- FPS counter showing reasonable value (~60)
- Map position and zoom level updating when panning/zooming
- Attribution text "(c) OSM, CartoDB" on the right
- No floating overlays for recording indicator or attribution
- Press Ctrl+R: verify blinking "REC Xs" appears in statusbar
- Dock panels and toolbar still render correctly
- Statusbar uses theme colors correctly

**Step 3: Final commit if any tweaks needed**

If visual adjustments were needed, commit them.
