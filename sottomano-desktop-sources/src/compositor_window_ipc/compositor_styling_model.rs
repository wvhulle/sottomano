use sottomano_toml_config::screen_edges::Position;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppWindowLayout {
    pub window_corner_radius_px: f32,
    pub gaps_px: f32,
}

impl Default for AppWindowLayout {
    fn default() -> Self {
        Self {
            window_corner_radius_px: 0.0,
            gaps_px: 16.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ShellBlurDepth {
    #[default]
    WindowsBehind,
    WallpaperOnly,
}

impl ShellBlurDepth {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            ShellBlurDepth::WindowsBehind => ShellBlurDepth::WallpaperOnly,
            ShellBlurDepth::WallpaperOnly => ShellBlurDepth::WindowsBehind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CompositorStyling {
    pub blur_depth: ShellBlurDepth,
    pub layout: AppWindowLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyledSurface {
    Panel,
    NotificationToasts,
    Backdrop(Position),
}

impl StyledSurface {
    pub const ALL: [Self; 4] = [
        Self::Panel,
        Self::NotificationToasts,
        Self::Backdrop(Position::Top),
        Self::Backdrop(Position::Bottom),
    ];

    #[must_use]
    pub fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "sottomano-panel",
            Self::NotificationToasts => "sottomano-notification-toasts",
            Self::Backdrop(Position::Top) => "sottomano-backdrop-top",
            Self::Backdrop(Position::Bottom) => "sottomano-backdrop-bottom",
            Self::Backdrop(Position::Left) => "sottomano-backdrop-left",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatedWindow {
    app_id: Option<String>,
    title_without_status_prefix: String,
    border_color: String,
}

impl HeatedWindow {
    #[must_use]
    pub fn titled(app_id: Option<String>, title: &str, border_color: String) -> Option<Self> {
        let title_without_status_prefix =
            title.trim_start_matches(|character: char| !character.is_alphanumeric());
        (!title_without_status_prefix.is_empty()).then(|| Self {
            app_id,
            title_without_status_prefix: title_without_status_prefix.to_owned(),
            border_color,
        })
    }

    #[must_use]
    pub fn app_id(&self) -> Option<&str> {
        self.app_id.as_deref()
    }

    #[must_use]
    pub fn title_without_status_prefix(&self) -> &str {
        &self.title_without_status_prefix
    }

    #[must_use]
    pub fn border_color(&self) -> &str {
        &self.border_color
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SwitchHeat {
    #[default]
    Hidden,
    Shown {
        focus_ring_color: String,
        neutral_border_color: String,
        windows: Vec<HeatedWindow>,
    },
}

impl SwitchHeat {
    #[must_use]
    pub fn heated_windows(&self) -> &[HeatedWindow] {
        match self {
            Self::Hidden => &[],
            Self::Shown { windows, .. } => windows,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylingControl {
    WindowCornerRadius,
    WindowGaps,
    ShellBlurDepth,
    SwitchHeatBorders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSupport {
    Available,
    Missing,
}

#[must_use]
pub fn escaped_for_regex(literal: &str) -> String {
    literal
        .chars()
        .filter(|character| !character.is_control())
        .flat_map(|character| {
            let metacharacter = "\\.+*?()|[]{}^$#&-~".contains(character);
            metacharacter.then_some('\\').into_iter().chain([character])
        })
        .collect()
}
