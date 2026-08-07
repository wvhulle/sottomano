pub mod idle;
pub mod launches;
pub mod panel;
pub mod power;
pub mod store;

pub(crate) use store::{BoxError, integer, open, text};

use crate::config::chrome::Chrome;

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub chrome: ChromeState,
    pub notifications: NotificationState,
}

#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ChromeState {
    pub floating: bool,
    pub blur_background: bool,
    pub disconnected_groups: bool,
    pub float_offset_px: f32,
    pub darken_chrome_factor: f32,
}

#[derive(Debug, Clone)]
pub struct NotificationState {
    pub dnd: bool,
    pub sound: bool,
}

impl Default for NotificationState {
    fn default() -> Self {
        Self {
            dnd: false,
            sound: true,
        }
    }
}

impl ChromeState {
    pub fn radius(&self, chrome: &Chrome) -> f32 {
        if self.floating {
            f32::from(chrome.corner_radius_px)
        } else {
            0.0
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn margin_offset_px(&self) -> i32 {
        if self.floating {
            self.float_offset_px as i32
        } else {
            0
        }
    }
}
