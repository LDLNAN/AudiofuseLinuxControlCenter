//! Utilitarian Studio Hardware Theme (`AGENTS.md` & `mousetool` specification).
//!
//! Strictly enforces:
//! - Dark Mode (default studio rack): Carbon `#141413` background, `#1e1e1c` panel surface,
//!   `#282826` recessed controls, `#2e2e2b` `1px solid` crisp dividers.
//! - Light Mode (warm studio paper): `#f6f5f2` background, `#ffffff` panel surface,
//!   `#eeede8` recessed controls, `#dcdad3` `1px solid` dividers.
//! - Functional accents only:
//!   - `#e05d30` / `#c84b20` for active/+48V/engaged states
//!   - `#2a9d8f` for lock/sync
//!   - `#e9c46a` for Solo/Pad/warn
//!   - `#e63946` for Mute/Clip/Peak
//! - Corner radius strictly `2px..4px` (`3.0`), `1px` structural strokes, zero drop shadows or gradients.

use egui::{Color32, CornerRadius, Stroke, Style, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StudioThemeMode {
    #[default]
    DarkCarbon,
    LightPaper,
}

impl StudioThemeMode {
    pub fn toggle(&mut self) {
        *self = match self {
            Self::DarkCarbon => Self::LightPaper,
            Self::LightPaper => Self::DarkCarbon,
        };
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::DarkCarbon => "DARK (#141413)",
            Self::LightPaper => "LIGHT (#F6F5F2)",
        }
    }
}

/// Complete studio color palette for immediate-mode `egui` rendering and headless PNG rendering.
#[derive(Debug, Clone, Copy)]
pub struct StudioPalette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub recessed: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent_orange: Color32,
    pub sync_teal: Color32,
    pub warn_amber: Color32,
    pub alert_red: Color32,
    pub meter_green: Color32,
    pub meter_amber: Color32,
    pub meter_red: Color32,
}

impl StudioPalette {
    pub const fn for_mode(mode: StudioThemeMode) -> Self {
        match mode {
            StudioThemeMode::DarkCarbon => Self {
                bg: Color32::from_rgb(0x14, 0x14, 0x13),
                surface: Color32::from_rgb(0x1e, 0x1e, 0x1c),
                surface_alt: Color32::from_rgb(0x23, 0x23, 0x20),
                recessed: Color32::from_rgb(0x28, 0x28, 0x26),
                border: Color32::from_rgb(0x2e, 0x2e, 0x2b),
                border_strong: Color32::from_rgb(0x42, 0x42, 0x3d),
                text_primary: Color32::from_rgb(0xe8, 0xe6, 0xdf),
                text_secondary: Color32::from_rgb(0xc2, 0xc0, 0xb8),
                text_muted: Color32::from_rgb(0x8c, 0x8a, 0x82),
                accent_orange: Color32::from_rgb(0xe0, 0x5d, 0x30),
                sync_teal: Color32::from_rgb(0x2a, 0x9d, 0x8f),
                warn_amber: Color32::from_rgb(0xe9, 0xc4, 0x6a),
                alert_red: Color32::from_rgb(0xe6, 0x39, 0x46),
                meter_green: Color32::from_rgb(0x2a, 0x9d, 0x8f),
                meter_amber: Color32::from_rgb(0xe9, 0xc4, 0x6a),
                meter_red: Color32::from_rgb(0xe6, 0x39, 0x46),
            },
            StudioThemeMode::LightPaper => Self {
                bg: Color32::from_rgb(0xf6, 0xf5, 0xf2),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_alt: Color32::from_rgb(0xf2, 0xf1, 0xec),
                recessed: Color32::from_rgb(0xee, 0xed, 0xe8),
                border: Color32::from_rgb(0xdc, 0xda, 0xd3),
                border_strong: Color32::from_rgb(0xc4, 0xc1, 0xb7),
                text_primary: Color32::from_rgb(0x16, 0x16, 0x15),
                text_secondary: Color32::from_rgb(0x3d, 0x3c, 0x38),
                text_muted: Color32::from_rgb(0x68, 0x66, 0x60),
                accent_orange: Color32::from_rgb(0xc8, 0x4b, 0x20),
                sync_teal: Color32::from_rgb(0x1f, 0x7a, 0x6e),
                warn_amber: Color32::from_rgb(0xc2, 0x8b, 0x16),
                alert_red: Color32::from_rgb(0xc9, 0x2a, 0x35),
                meter_green: Color32::from_rgb(0x24, 0x82, 0x77),
                meter_amber: Color32::from_rgb(0xd4, 0x9a, 0x22),
                meter_red: Color32::from_rgb(0xc9, 0x2a, 0x35),
            },
        }
    }

    pub fn apply_to_ctx(&self, ctx: &egui::Context, mode: StudioThemeMode) {
        let mut style = Style::default();
        let mut visuals = match mode {
            StudioThemeMode::DarkCarbon => Visuals::dark(),
            StudioThemeMode::LightPaper => Visuals::light(),
        };

        let radius = CornerRadius::same(3);
        visuals.panel_fill = self.bg;
        visuals.window_fill = self.surface;
        visuals.extreme_bg_color = self.recessed;
        visuals.faint_bg_color = self.surface_alt;
        visuals.window_corner_radius = radius;
        visuals.menu_corner_radius = radius;
        visuals.window_stroke = Stroke::new(1.0_f32, self.border);
        visuals.window_shadow = egui::epaint::Shadow::NONE;
        visuals.popup_shadow = egui::epaint::Shadow::NONE;

        visuals.widgets.noninteractive.bg_fill = self.surface;
        visuals.widgets.noninteractive.weak_bg_fill = self.surface;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.noninteractive.corner_radius = radius;

        visuals.widgets.inactive.bg_fill = self.recessed;
        visuals.widgets.inactive.weak_bg_fill = self.recessed;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.inactive.corner_radius = radius;

        visuals.widgets.hovered.bg_fill = self.surface_alt;
        visuals.widgets.hovered.weak_bg_fill = self.surface_alt;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, self.accent_orange);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.hovered.corner_radius = radius;

        visuals.widgets.active.bg_fill = self.accent_orange;
        visuals.widgets.active.weak_bg_fill = self.accent_orange;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, self.accent_orange);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.active.corner_radius = radius;

        visuals.selection.bg_fill = self.accent_orange;
        visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);

        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(4.0, 4.0);
        style.spacing.button_padding = egui::vec2(5.0, 2.0);
        style.spacing.window_margin = egui::Margin::same(6);
        ctx.set_style(style);
    }
}
