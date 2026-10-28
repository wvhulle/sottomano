use iced::{Color, Theme, widget::text};
use sottomano_config::severity::Severity;

use crate::widgets::{
    configured_theme::{
        danger_color, primary_color, secondary_text_color, text_color, warning_color,
    },
    phosphor::Weight,
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
    Live,
    Critical,
}

impl WidgetState {
    #[must_use]
    pub const fn of_toggle(on: bool) -> Self {
        if on {
            WidgetState::On
        } else {
            WidgetState::Off
        }
    }

    #[must_use]
    pub const fn weight(self) -> Weight {
        match self {
            WidgetState::Failed => Weight::Thin,
            WidgetState::Off => Weight::Light,
            WidgetState::Idle => Weight::Regular,
            WidgetState::Pending => Weight::Duotone,
            WidgetState::Warning => Weight::Bold,
            WidgetState::On | WidgetState::Live | WidgetState::Critical => Weight::Fill,
        }
    }

    #[must_use]
    pub fn color(self, theme: &Theme) -> Color {
        match self {
            WidgetState::Failed | WidgetState::Critical => danger_color(theme),
            WidgetState::Off => secondary_text_color(theme),
            WidgetState::Idle | WidgetState::On => text_color(theme),
            WidgetState::Live => primary_color(theme),
            WidgetState::Pending | WidgetState::Warning => warning_color(theme),
        }
    }

    #[must_use]
    pub const fn text_style(self) -> fn(&Theme) -> text::Style {
        match self {
            WidgetState::Failed | WidgetState::Critical => text::danger,
            WidgetState::Off => text::secondary,
            WidgetState::Idle | WidgetState::On => text::base,
            WidgetState::Live => text::primary,
            WidgetState::Pending | WidgetState::Warning => text::warning,
        }
    }

    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            WidgetState::Failed | WidgetState::Critical => Severity::Critical,
            WidgetState::Warning => Severity::Warning,
            WidgetState::Off
            | WidgetState::Idle
            | WidgetState::Pending
            | WidgetState::On
            | WidgetState::Live => Severity::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateTint {
    pub state: WidgetState,
    pub opacity: f32,
}

impl StateTint {
    #[must_use]
    pub const fn opaque(state: WidgetState) -> Self {
        Self {
            state,
            opacity: 1.0,
        }
    }

    #[must_use]
    pub fn color(self, theme: &Theme) -> Color {
        self.state.color(theme).scale_alpha(self.opacity)
    }

    pub fn text_style(self) -> impl Fn(&Theme) -> text::Style + Copy {
        move |theme| text::Style {
            color: Some(self.color(theme)),
        }
    }
}

impl From<Severity> for WidgetState {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Low => WidgetState::Off,
            Severity::Normal => WidgetState::Idle,
            Severity::Warning => WidgetState::Warning,
            Severity::Critical => WidgetState::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_config::theme::palette::Palette;

    use super::*;
    use crate::widgets::configured_theme::configured_theme;

    #[test]
    fn severity_states_share_the_severity_colours() {
        let theme = configured_theme(&Palette::default());
        assert_eq!(WidgetState::Failed.color(&theme), danger_color(&theme));
        assert_eq!(WidgetState::Critical.color(&theme), danger_color(&theme));
        assert_eq!(WidgetState::Warning.color(&theme), warning_color(&theme));
        assert_eq!(WidgetState::Off.color(&theme), secondary_text_color(&theme));
        assert_eq!(WidgetState::Live.color(&theme), primary_color(&theme));
    }

    #[test]
    fn each_state_text_style_uses_its_colour() {
        let theme = configured_theme(&Palette::default());
        [
            WidgetState::Failed,
            WidgetState::Off,
            WidgetState::Idle,
            WidgetState::Live,
            WidgetState::Warning,
        ]
        .into_iter()
        .for_each(|state| {
            assert_eq!(
                (state.text_style())(&theme).color,
                Some(state.color(&theme)),
                "{state:?}"
            );
        });
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
