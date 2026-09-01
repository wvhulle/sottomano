pub mod idle;
pub mod launches;
pub mod panel;
pub mod power;
pub mod store;

pub(crate) use store::{StoreError, integer, open, text};

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub chrome: ChromeState,
    pub notifications: NotificationState,
}

#[derive(Debug, Clone)]
pub struct ChromeState {
    pub blur_background: bool,
    pub disconnected_groups: bool,
    pub auto_collapse_groups: bool,
    pub corner_radius_px: f32,
    pub float_offset_px: f32,
    pub chrome_tint_factor: f32,
}

impl Default for ChromeState {
    fn default() -> Self {
        Self {
            blur_background: false,
            disconnected_groups: false,
            auto_collapse_groups: true,
            corner_radius_px: 0.0,
            float_offset_px: 0.0,
            chrome_tint_factor: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotificationState {
    pub dnd: bool,
    pub sound: bool,
    pub spatial_layout: bool,
}

impl Default for NotificationState {
    fn default() -> Self {
        Self {
            dnd: false,
            sound: true,
            spatial_layout: true,
        }
    }
}

impl ChromeState {
    pub fn radius(&self) -> f32 {
        self.corner_radius_px
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn margin_offset_px(&self) -> i32 {
        self.float_offset_px as i32
    }
}
