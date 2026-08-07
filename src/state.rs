pub mod idle;
pub mod launches;
pub mod panel;
pub mod power;
pub mod store;

use std::process::Command;

use panel::PanelSettings;
pub(crate) use store::{BoxError, integer, open, text};

use crate::config::layout::Layout;

#[derive(Debug, Clone)]
pub struct Session {
    pub chrome: ChromeState,
    pub notifications: NotificationState,
}

#[derive(Debug, Clone)]
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
    pub sound_theme: Option<String>,
}

impl Session {
    pub fn initial(layout: &Layout) -> Self {
        Self {
            chrome: ChromeState {
                floating: layout.floating,
                blur_background: layout.blur_background,
                disconnected_groups: layout.disconnected_groups,
                float_offset_px: 0.0,
                darken_chrome_factor: 0.0,
            },
            notifications: NotificationState {
                dnd: false,
                sound: true,
                sound_theme: None,
            },
        }
    }

    pub fn absorb_saved(&mut self, saved: &PanelSettings) {
        self.chrome.absorb_saved(saved);
        self.notifications.absorb_saved(saved);
    }
}

impl ChromeState {
    fn absorb_saved(&mut self, saved: &PanelSettings) {
        if let Some(floating) = saved.floating {
            self.floating = floating;
        }
        if let Some(offset) = saved.float_offset_px {
            self.float_offset_px = offset;
        }
        if let Some(disconnected) = saved.disconnected_groups {
            self.disconnected_groups = disconnected;
        }
        if let Some(blur) = saved.blur_background {
            self.blur_background = blur;
        }
        if let Some(factor) = saved.darken_chrome_factor {
            self.darken_chrome_factor = factor;
        }
    }

    pub fn radius(&self, layout: &Layout) -> f32 {
        if self.floating {
            f32::from(layout.corner_radius_px)
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

impl NotificationState {
    fn absorb_saved(&mut self, saved: &PanelSettings) {
        self.dnd = saved.dnd;
        if let Some(sound) = saved.sound {
            self.sound = sound;
        }
    }
}

pub(crate) fn session_sound_theme() -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.sound", "theme-name"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let theme = value.trim().trim_matches('\'');
    (!theme.is_empty()).then(|| theme.to_owned())
}
