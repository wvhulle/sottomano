use iced::Color;

use crate::{
    config::palette::Palette,
    widgets::{bar_widget::Level, phosphor::Weight},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WidgetState {
    Failed,
    Off,
    #[default]
    Idle,
    Pending,
    Warning,
    On,
    Critical,
}

impl WidgetState {
    pub const fn of_toggle(on: bool) -> Self {
        if on {
            WidgetState::On
        } else {
            WidgetState::Off
        }
    }

    pub const fn weight(self) -> Weight {
        match self {
            WidgetState::Failed => Weight::Thin,
            WidgetState::Off => Weight::Light,
            WidgetState::Idle => Weight::Regular,
            WidgetState::Pending => Weight::Duotone,
            WidgetState::Warning => Weight::Bold,
            WidgetState::On | WidgetState::Critical => Weight::Fill,
        }
    }

    pub const fn color(self, palette: &Palette) -> Color {
        match self {
            WidgetState::Failed | WidgetState::Critical => palette.urgent,
            WidgetState::Off => palette.inactive,
            WidgetState::Idle | WidgetState::Pending | WidgetState::On => palette.bar_foreground,
            WidgetState::Warning => palette.warning,
        }
    }

    pub const fn severity(self) -> Level {
        match self {
            WidgetState::Failed | WidgetState::Critical => Level::Critical,
            WidgetState::Warning => Level::Warning,
            WidgetState::Off | WidgetState::Idle | WidgetState::Pending | WidgetState::On => {
                Level::Ok
            }
        }
    }
}

impl From<Level> for WidgetState {
    fn from(level: Level) -> Self {
        match level {
            Level::Ok => WidgetState::Idle,
            Level::Warning => WidgetState::Warning,
            Level::Critical => WidgetState::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_states_share_the_severity_colours() {
        let palette = Palette::default();
        assert_eq!(WidgetState::Failed.color(&palette), palette.urgent);
        assert_eq!(WidgetState::Critical.color(&palette), palette.urgent);
        assert_eq!(WidgetState::Warning.color(&palette), palette.warning);
        assert_eq!(WidgetState::Off.color(&palette), palette.inactive);
    }

    #[test]
    fn each_state_below_critical_has_its_own_weight() {
        let weights = [
            WidgetState::Failed,
            WidgetState::Off,
            WidgetState::Idle,
            WidgetState::Pending,
            WidgetState::Warning,
            WidgetState::On,
        ]
        .map(WidgetState::weight);
        assert!(
            weights
                .iter()
                .enumerate()
                .all(|(index, weight)| !weights[index + 1..].contains(weight))
        );
        assert_eq!(WidgetState::Critical.weight(), WidgetState::On.weight());
    }

    #[test]
    fn toggles_map_to_on_and_off() {
        assert_eq!(WidgetState::of_toggle(true), WidgetState::On);
        assert_eq!(WidgetState::of_toggle(false), WidgetState::Off);
    }
}
