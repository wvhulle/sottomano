pub mod charge_power_curve;
pub mod dark_mode;
pub mod idle;
pub mod launches;
pub mod panel;
pub mod power;
pub mod rate_limit;
pub mod store;

use iced::border::Radius;
pub(crate) use store::{Columns, StoreError, open};

#[derive(Debug, Clone, Default)]
pub struct PanelSettings {
    pub layout: PanelLayout,
    pub toasts: ToastPreferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacesPlacement {
    Sidebar,
    BottomPanel,
}

impl WorkspacesPlacement {
    pub fn toggled(self) -> Self {
        match self {
            Self::Sidebar => Self::BottomPanel,
            Self::BottomPanel => Self::Sidebar,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowStyle {
    pub cast: bool,
    pub softness_px: f32,
    pub spread_px: f32,
    pub opacity: f32,
}

impl Default for ShadowStyle {
    fn default() -> Self {
        Self {
            cast: true,
            softness_px: 20.0,
            spread_px: 4.0,
            opacity: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NiriStyling {
    pub blurred: bool,
    pub xray: bool,
    pub shadow: ShadowStyle,
}

#[derive(Debug, Clone)]
pub struct PanelLayout {
    pub niri: NiriStyling,
    pub disconnected_groups: bool,
    pub auto_collapse_groups: bool,
    pub workspaces_placement: WorkspacesPlacement,
    pub float_offset_px: f32,
    pub tint_factor: f32,
    pub corner_radius: Radius,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            niri: NiriStyling::default(),
            disconnected_groups: false,
            auto_collapse_groups: true,
            workspaces_placement: WorkspacesPlacement::Sidebar,
            float_offset_px: 0.0,
            tint_factor: 0.0,
            corner_radius: Radius::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToastPreferences {
    pub do_not_disturb: bool,
    pub sound: bool,
    pub spatial_layout: bool,
}

impl Default for ToastPreferences {
    fn default() -> Self {
        Self {
            do_not_disturb: false,
            sound: true,
            spatial_layout: true,
        }
    }
}

impl PanelLayout {
    #[allow(clippy::cast_possible_truncation)]
    pub fn margin_offset_px(&self) -> i32 {
        self.float_offset_px as i32
    }

    pub fn keep_niri_look_of(&mut self, current: &PanelLayout) {
        self.niri = current.niri;
        self.corner_radius = current.corner_radius;
    }
}
