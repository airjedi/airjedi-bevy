//! 3D View Mode Module
//!
//! Provides a tilted perspective view showing aircraft at their altitudes
//! above a flat map plane. Uses Camera2d with perspective projection so that
//! all existing 2D content (tiles, trails, sprites) renders correctly.
//! Aircraft altitude is shown by adjusting sprite Z positions.

use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy_egui::{egui, EguiContexts};

// Constants for 3D view
const TRANSITION_DURATION: f32 = 0.5;
const DEFAULT_PITCH: f32 = 25.0;
const DEFAULT_CAMERA_ALTITUDE: f32 = 10000.0;
const MIN_PITCH: f32 = 15.0;
const MAX_PITCH: f32 = 89.0;
const MIN_CAMERA_ALTITUDE: f32 = 1000.0;
const MAX_CAMERA_ALTITUDE: f32 = 60000.0;
const ALTITUDE_EXAGGERATION: f32 = 2.0;

/// Scale factor to convert altitude/distance values to pixel-space.
const PIXEL_SCALE: f32 = 20.0;

/// View mode for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Map2D,
    Perspective3D,
}

/// Transition state between view modes
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TransitionState {
    #[default]
    Idle,
    TransitioningTo3D { progress: f32 },
    TransitioningTo2D { progress: f32 },
}

/// Resource for 3D view state
#[derive(Resource)]
pub struct View3DState {
    pub mode: ViewMode,
    pub transition: TransitionState,
    pub camera_pitch: f32,
    pub camera_altitude: f32,
    pub camera_yaw: f32,
    pub altitude_scale: f32,
    pub show_panel: bool,
    /// Saved 2D camera position (pixel coords) when entering 3D mode
    pub saved_2d_center: Vec2,
}

impl Default for View3DState {
    fn default() -> Self {
        Self {
            mode: ViewMode::Map2D,
            transition: TransitionState::Idle,
            camera_pitch: DEFAULT_PITCH,
            camera_altitude: DEFAULT_CAMERA_ALTITUDE,
            camera_yaw: 0.0,
            altitude_scale: ALTITUDE_EXAGGERATION,
            show_panel: false,
            saved_2d_center: Vec2::ZERO,
        }
    }
}

impl View3DState {
    pub fn is_3d_active(&self) -> bool {
        matches!(self.mode, ViewMode::Perspective3D)
            || matches!(self.transition, TransitionState::TransitioningTo3D { .. })
    }

    pub fn is_transitioning(&self) -> bool {
        !matches!(self.transition, TransitionState::Idle)
    }

    /// Convert altitude in feet to pixel-space Z offset
    pub fn altitude_to_z(&self, altitude_feet: i32) -> f32 {
        // Convert feet to km, then scale to pixel space
        let alt_km = altitude_feet as f32 * 0.3048 / 1000.0;
        alt_km * PIXEL_SCALE * self.altitude_scale
    }

    /// Convert camera altitude in feet to pixel-space distance
    pub fn altitude_to_distance(&self) -> f32 {
        let alt_km = self.camera_altitude * 0.3048 / 1000.0;
        alt_km * PIXEL_SCALE * self.altitude_scale
    }

    /// Calculate the 3D camera transform orbiting around a center point in pixel space
    fn calculate_camera_transform(&self, center: Vec3) -> Transform {
        let pitch_rad = self.camera_pitch.to_radians();
        let yaw_rad = self.camera_yaw.to_radians();

        let effective_distance = self.altitude_to_distance();
        let horizontal_dist = effective_distance * pitch_rad.cos();
        let vertical_dist = effective_distance * pitch_rad.sin();

        // Z is "up" (altitude above map plane), orbit in XY plane.
        // At yaw=0, camera is south of center looking north (so north stays up on screen).
        let camera_pos = Vec3::new(
            center.x - horizontal_dist * yaw_rad.sin(),
            center.y - horizontal_dist * yaw_rad.cos(),
            center.z + vertical_dist,
        );

        Transform::from_translation(camera_pos).looking_at(center, Vec3::Z)
    }
}

/// System to toggle 3D view mode with smooth transition
pub fn toggle_3d_view(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<View3DState>,
    mut contexts: EguiContexts,
    camera_query: Query<&Transform, With<Camera2d>>,
) {
    let egui_wants_input = contexts.ctx_mut()
        .map(|ctx| ctx.wants_keyboard_input())
        .unwrap_or(false);

    if egui_wants_input {
        return;
    }

    if keyboard.just_pressed(KeyCode::Digit3) {
        // Don't start new transition if one is in progress
        if state.is_transitioning() {
            return;
        }

        match state.mode {
            ViewMode::Map2D => {
                // Save current 2D camera center before transitioning
                if let Ok(cam_transform) = camera_query.single() {
                    state.saved_2d_center = Vec2::new(
                        cam_transform.translation.x,
                        cam_transform.translation.y,
                    );
                }
                state.transition = TransitionState::TransitioningTo3D { progress: 0.0 };
                state.show_panel = true;
                info!("Starting transition to 3D view");
            }
            ViewMode::Perspective3D => {
                state.transition = TransitionState::TransitioningTo2D { progress: 0.0 };
                info!("Starting transition to 2D view");
            }
        }
    }
}

/// System to render 3D view settings panel
pub fn render_3d_view_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<View3DState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    if !state.show_panel {
        return;
    }

    egui::Window::new("3D View Settings")
        .collapsible(true)
        .resizable(false)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.colored_label(
                egui::Color32::YELLOW,
                "3D View - Experimental"
            );

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Mode:");
                let mode_text = match state.mode {
                    ViewMode::Map2D => "2D Map",
                    ViewMode::Perspective3D => "3D Perspective",
                };
                ui.strong(mode_text);
            });

            ui.separator();
            ui.heading("Camera Settings");

            ui.horizontal(|ui| {
                ui.label("Pitch:");
                ui.add(egui::Slider::new(&mut state.camera_pitch, MIN_PITCH..=MAX_PITCH).suffix("°"));
            });

            ui.horizontal(|ui| {
                ui.label("Altitude:");
                ui.add(egui::Slider::new(&mut state.camera_altitude, MIN_CAMERA_ALTITUDE..=MAX_CAMERA_ALTITUDE).suffix(" ft"));
            });

            ui.horizontal(|ui| {
                ui.label("Yaw:");
                ui.add(egui::Slider::new(&mut state.camera_yaw, 0.0..=360.0).suffix("°"));
            });

            ui.separator();
            ui.heading("Altitude");

            ui.horizontal(|ui| {
                ui.label("Exaggeration:");
                ui.add(egui::Slider::new(&mut state.altitude_scale, 0.5..=50.0).suffix("x"));
            });

            ui.separator();
            ui.label(
                egui::RichText::new("Press '3' to toggle view mode")
                    .size(11.0)
                    .color(egui::Color32::GRAY)
            );
        });
}

/// System to animate the view transition
pub fn animate_view_transition(
    time: Res<Time>,
    mut state: ResMut<View3DState>,
) {
    let delta = time.delta_secs() / TRANSITION_DURATION;

    match state.transition {
        TransitionState::TransitioningTo3D { progress } => {
            let new_progress = (progress + delta).min(1.0);
            if new_progress >= 1.0 {
                state.mode = ViewMode::Perspective3D;
                state.transition = TransitionState::Idle;
                info!("Transition to 3D complete");
            } else {
                state.transition = TransitionState::TransitioningTo3D { progress: new_progress };
            }
        }
        TransitionState::TransitioningTo2D { progress } => {
            let new_progress = (progress + delta).min(1.0);
            // Don't finalize here — let update_3d_camera reset the camera
            // before clearing the transition state, avoiding the one-frame
            // race where the early return skips the camera reset.
            state.transition = TransitionState::TransitioningTo2D { progress: new_progress };
        }
        TransitionState::Idle => {}
    }
}

/// System to update Camera2d for 3D perspective view.
/// Works entirely in pixel-coordinate space so tiles, trails, and aircraft all align.
pub fn update_3d_camera(
    mut state: ResMut<View3DState>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    window_query: Query<&Window>,
    zoom_state: Res<crate::ZoomState>,
) {
    // Only run when transitioning or in 3D mode
    if matches!(state.mode, ViewMode::Map2D) && !state.is_transitioning() {
        return;
    }

    let Ok((mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    // Get transition progress (0 = 2D, 1 = 3D)
    let t = match state.transition {
        TransitionState::Idle => {
            match state.mode {
                ViewMode::Map2D => 0.0,
                ViewMode::Perspective3D => 1.0,
            }
        }
        TransitionState::TransitioningTo3D { progress } => smooth_step(progress),
        TransitionState::TransitioningTo2D { progress } => smooth_step(1.0 - progress),
    };

    // Fixed endpoints for interpolation
    let pos_2d = Vec3::new(state.saved_2d_center.x, state.saved_2d_center.y, 0.0);
    let center = pos_2d;
    let transform_3d = state.calculate_camera_transform(center);

    // Compute the perspective altitude that produces the same visible area as
    // the current orthographic view. At this height with a 60° FOV looking
    // straight down, the perspective and orthographic views match exactly,
    // making the projection switch at the endpoints visually seamless.
    let base_fov = 60.0_f32.to_radians();
    let matching_z = if let Ok(window) = window_query.single() {
        window.height() / (2.0 * zoom_state.camera_zoom * (base_fov / 2.0).tan())
    } else {
        transform_3d.translation.z * 0.5
    };

    if t < 0.001 {
        // Pure 2D mode - restore orthographic projection, position, and rotation
        *projection = Projection::Orthographic(OrthographicProjection::default_2d());
        transform.translation = pos_2d;
        transform.rotation = Quat::IDENTITY;

        // Finalize the transition now that the camera has been reset
        if matches!(state.transition, TransitionState::TransitioningTo2D { .. }) {
            state.mode = ViewMode::Map2D;
            state.transition = TransitionState::Idle;
            info!("Transition to 2D complete");
        }
        return;
    }

    if t > 0.999 {
        // Pure 3D mode
        *transform = transform_3d;
        *projection = Projection::Perspective(PerspectiveProjection {
            fov: base_fov,
            ..default()
        });
    } else {
        // Straight-line transition using perspective throughout. The 2D
        // endpoint is placed at matching_z — the altitude where a 60° FOV
        // perspective camera looking straight down shows the same area as the
        // orthographic view. This makes the ortho↔perspective switch at the
        // animation boundaries visually seamless, and the camera follows a
        // natural straight-line path between the overhead and orbit positions.
        let pos_match = Vec3::new(pos_2d.x, pos_2d.y, matching_z);

        transform.translation = pos_match.lerp(transform_3d.translation, t);
        transform.rotation = Quat::IDENTITY.slerp(transform_3d.rotation, t);
        *projection = Projection::Perspective(PerspectiveProjection {
            fov: base_fov,
            ..default()
        });
    }
}

/// Smooth step function for easing transitions
fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

const ORBIT_SENSITIVITY: f32 = 0.3;
const PITCH_SCROLL_SENSITIVITY: f32 = 2.0;
const ALTITUDE_SCROLL_SENSITIVITY: f32 = 1000.0;

/// System to handle 3D camera controls (orbit, pitch, distance)
pub fn handle_3d_camera_controls(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut state: ResMut<View3DState>,
    mut contexts: EguiContexts,
) {
    // Only active in 3D mode
    if !matches!(state.mode, ViewMode::Perspective3D) {
        // Clear events to avoid accumulation
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    // Don't process if transitioning
    if state.is_transitioning() {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    // Check if egui wants input
    let egui_wants_input = contexts.ctx_mut()
        .map(|ctx| ctx.wants_pointer_input() || ctx.wants_keyboard_input())
        .unwrap_or(false);

    if egui_wants_input {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    // Left drag = orbit (yaw)
    if mouse_button.pressed(MouseButton::Left) {
        for event in mouse_motion.read() {
            state.camera_yaw += event.delta.x * ORBIT_SENSITIVITY;
            if state.camera_yaw < 0.0 {
                state.camera_yaw += 360.0;
            } else if state.camera_yaw >= 360.0 {
                state.camera_yaw -= 360.0;
            }
        }
    } else {
        mouse_motion.clear();
    }

    // Scroll = altitude, Shift+Scroll = pitch
    for event in scroll_events.read() {
        let scroll_y = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.01,
        };

        if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
            state.camera_pitch = (state.camera_pitch + scroll_y * PITCH_SCROLL_SENSITIVITY)
                .clamp(MIN_PITCH, MAX_PITCH);
        } else {
            state.camera_altitude = (state.camera_altitude - scroll_y * ALTITUDE_SCROLL_SENSITIVITY)
                .clamp(MIN_CAMERA_ALTITUDE, MAX_CAMERA_ALTITUDE);
        }
    }
}

/// System to adjust aircraft sprite Z positions based on altitude in 3D mode.
/// In 2D mode, aircraft Z is the fixed layer constant. In 3D mode, Z represents altitude
/// so aircraft appear at different heights above the map when viewed from a tilted camera.
pub fn update_aircraft_altitude_z(
    state: Res<View3DState>,
    mut aircraft_query: Query<(&crate::Aircraft, &mut Transform), Without<crate::AircraftLabel>>,
    mut label_query: Query<(&crate::AircraftLabel, &mut Visibility)>,
) {
    if state.is_3d_active() {
        for (aircraft, mut transform) in aircraft_query.iter_mut() {
            let alt = aircraft.altitude.unwrap_or(0);
            transform.translation.z = state.altitude_to_z(alt);
        }
        // Hide labels in 3D mode (they don't position well in perspective)
        for (_label, mut vis) in label_query.iter_mut() {
            *vis = Visibility::Hidden;
        }
    } else if !state.is_transitioning() {
        // Restore aircraft to fixed Z layer in 2D mode
        for (_aircraft, mut transform) in aircraft_query.iter_mut() {
            transform.translation.z = crate::constants::AIRCRAFT_Z_LAYER;
        }
        // Restore label visibility
        for (_label, mut vis) in label_query.iter_mut() {
            if *vis == Visibility::Hidden {
                *vis = Visibility::Inherited;
            }
        }
    }
}

/// Plugin for 3D view functionality
pub struct View3DPlugin;

impl Plugin for View3DPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<View3DState>()
            .add_systems(Update, (
                toggle_3d_view,
                animate_view_transition,
                handle_3d_camera_controls,
                update_3d_camera.after(animate_view_transition),
            ))
            .add_systems(Update, update_aircraft_altitude_z);
        // 3D view settings panel is rendered via the consolidated Tools window (tools_window.rs)
    }
}
