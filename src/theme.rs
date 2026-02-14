use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use catppuccin::FlavorName;

/// Convert a catppuccin color to a bevy Color via its RGB values.
fn cat_to_bevy(c: &catppuccin::Color) -> Color {
    Color::srgb(
        c.rgb.r as f32 / 255.0,
        c.rgb.g as f32 / 255.0,
        c.rgb.b as f32 / 255.0,
    )
}

/// Central theme resource for the application.
///
/// Wraps a catppuccin flavor and provides accessor methods returning
/// `bevy::color::Color` and `egui::Color32` values from the active palette.
#[derive(Resource)]
pub struct AppTheme {
    active_flavor: FlavorName,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self {
            active_flavor: FlavorName::Mocha,
        }
    }
}

impl AppTheme {
    pub fn flavor(&self) -> FlavorName {
        self.active_flavor
    }

    pub fn set_flavor(&mut self, flavor: FlavorName) {
        self.active_flavor = flavor;
    }

    fn colors(&self) -> &catppuccin::FlavorColors {
        &catppuccin::PALETTE.get_flavor(self.active_flavor).colors
    }

    // -- Background / surface colors --

    pub fn base(&self) -> Color { cat_to_bevy(&self.colors().base) }
    pub fn mantle(&self) -> Color { cat_to_bevy(&self.colors().mantle) }
    pub fn crust(&self) -> Color { cat_to_bevy(&self.colors().crust) }
    pub fn surface0(&self) -> Color { cat_to_bevy(&self.colors().surface0) }
    pub fn surface1(&self) -> Color { cat_to_bevy(&self.colors().surface1) }
    pub fn surface2(&self) -> Color { cat_to_bevy(&self.colors().surface2) }

    // -- Text colors --

    pub fn text(&self) -> Color { cat_to_bevy(&self.colors().text) }
    pub fn subtext0(&self) -> Color { cat_to_bevy(&self.colors().subtext0) }
    pub fn subtext1(&self) -> Color { cat_to_bevy(&self.colors().subtext1) }

    // -- Overlay colors --

    pub fn overlay0(&self) -> Color { cat_to_bevy(&self.colors().overlay0) }
    pub fn overlay1(&self) -> Color { cat_to_bevy(&self.colors().overlay1) }
    pub fn overlay2(&self) -> Color { cat_to_bevy(&self.colors().overlay2) }

    // -- Accent colors --

    pub fn blue(&self) -> Color { cat_to_bevy(&self.colors().blue) }
    pub fn green(&self) -> Color { cat_to_bevy(&self.colors().green) }
    pub fn red(&self) -> Color { cat_to_bevy(&self.colors().red) }
    pub fn yellow(&self) -> Color { cat_to_bevy(&self.colors().yellow) }
    pub fn peach(&self) -> Color { cat_to_bevy(&self.colors().peach) }
    pub fn mauve(&self) -> Color { cat_to_bevy(&self.colors().mauve) }
    pub fn teal(&self) -> Color { cat_to_bevy(&self.colors().teal) }
    pub fn sky(&self) -> Color { cat_to_bevy(&self.colors().sky) }
    pub fn lavender(&self) -> Color { cat_to_bevy(&self.colors().lavender) }
    pub fn rosewater(&self) -> Color { cat_to_bevy(&self.colors().rosewater) }

    // -- egui theme --

    pub fn egui_theme(&self) -> catppuccin_egui::Theme {
        match self.active_flavor {
            FlavorName::Latte => catppuccin_egui::LATTE,
            FlavorName::Frappe => catppuccin_egui::FRAPPE,
            FlavorName::Macchiato => catppuccin_egui::MACCHIATO,
            FlavorName::Mocha => catppuccin_egui::MOCHA,
        }
    }
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

/// All available flavor names for iteration in UI.
pub const ALL_FLAVORS: &[FlavorName] = &[
    FlavorName::Latte,
    FlavorName::Frappe,
    FlavorName::Macchiato,
    FlavorName::Mocha,
];

pub fn flavor_display_name(flavor: FlavorName) -> &'static str {
    match flavor {
        FlavorName::Latte => "Latte",
        FlavorName::Frappe => "Frappe",
        FlavorName::Macchiato => "Macchiato",
        FlavorName::Mocha => "Mocha",
    }
}

/// System that applies the catppuccin egui theme whenever `AppTheme` changes.
pub fn apply_egui_theme(
    theme: Res<AppTheme>,
    mut contexts: EguiContexts,
) {
    if !theme.is_changed() {
        return;
    }
    if let Ok(ctx) = contexts.ctx_mut() {
        catppuccin_egui::set_theme(ctx, theme.egui_theme());
    }
}
