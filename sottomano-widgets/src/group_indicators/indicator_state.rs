use corredo::theming::{
    danger_color, primary_color, secondary_text_color, text_color, warning_color,
};
use iced::{Color, Theme, widget::text};
use sottomano_desktop_sources::{
    container_run_state::ContainerRunState,
    media_playback_audio::mpris_players::{LoopMode, QueueOrder},
    network_connectivity::{network_manager_proxies::LinkPhase, tailscale_status::ExitNodeOffer},
};
use sottomano_toml_config::severity::Severity;

use crate::widgets::phosphor::Weight;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IndicatorState {
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

impl IndicatorState {
    #[must_use]
    pub const fn weight(self) -> Weight {
        match self {
            IndicatorState::Failed => Weight::Thin,
            IndicatorState::Off => Weight::Light,
            IndicatorState::Pending => Weight::Duotone,
            IndicatorState::Idle => Weight::Regular,
            IndicatorState::Warning => Weight::Bold,
            IndicatorState::On | IndicatorState::Live | IndicatorState::Critical => Weight::Fill,
        }
    }

    #[must_use]
    pub fn color(self, theme: &Theme) -> Color {
        match self {
            IndicatorState::Failed | IndicatorState::Critical => danger_color(theme),
            IndicatorState::Off => secondary_text_color(theme),
            IndicatorState::Idle | IndicatorState::On => text_color(theme),
            IndicatorState::Live => primary_color(theme),
            IndicatorState::Pending | IndicatorState::Warning => warning_color(theme),
        }
    }

    #[must_use]
    pub const fn text_style(self) -> fn(&Theme) -> text::Style {
        match self {
            IndicatorState::Failed | IndicatorState::Critical => text::danger,
            IndicatorState::Off => text::secondary,
            IndicatorState::Idle | IndicatorState::On => text::base,
            IndicatorState::Live => text::primary,
            IndicatorState::Pending | IndicatorState::Warning => text::warning,
        }
    }

    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            IndicatorState::Failed | IndicatorState::Critical => Severity::Critical,
            IndicatorState::Warning => Severity::Warning,
            IndicatorState::Off
            | IndicatorState::Idle
            | IndicatorState::Pending
            | IndicatorState::On
            | IndicatorState::Live => Severity::Normal,
        }
    }
}

impl From<LinkPhase> for IndicatorState {
    fn from(link_state: LinkPhase) -> Self {
        match link_state {
            LinkPhase::Up => IndicatorState::On,
            LinkPhase::Switching => IndicatorState::Pending,
            LinkPhase::Down => IndicatorState::Off,
        }
    }
}

impl From<ContainerRunState> for IndicatorState {
    fn from(run_state: ContainerRunState) -> Self {
        match run_state {
            ContainerRunState::Running => IndicatorState::On,
            ContainerRunState::Stopped => IndicatorState::Off,
        }
    }
}

impl From<ExitNodeOffer> for IndicatorState {
    fn from(exit_offer: ExitNodeOffer) -> Self {
        match exit_offer {
            ExitNodeOffer::NotOffered => IndicatorState::Off,
            ExitNodeOffer::AwaitingApproval => IndicatorState::Pending,
            ExitNodeOffer::Approved => IndicatorState::On,
        }
    }
}

impl From<QueueOrder> for IndicatorState {
    fn from(queue_order: QueueOrder) -> Self {
        match queue_order {
            QueueOrder::Shuffled => IndicatorState::On,
            QueueOrder::InOrder => IndicatorState::Off,
        }
    }
}

impl From<LoopMode> for IndicatorState {
    fn from(loop_mode: LoopMode) -> Self {
        match loop_mode {
            LoopMode::Off => IndicatorState::Off,
            LoopMode::Track | LoopMode::Playlist => IndicatorState::On,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndicatorStateTint {
    pub state: IndicatorState,
    pub opacity: f32,
}

impl IndicatorStateTint {
    #[must_use]
    pub const fn opaque(state: IndicatorState) -> Self {
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

impl From<Severity> for IndicatorState {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Low => IndicatorState::Off,
            Severity::Normal => IndicatorState::Idle,
            Severity::Warning => IndicatorState::Warning,
            Severity::Critical => IndicatorState::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_toml_config::theme::palette::Palette;

    use super::*;
    use crate::widgets::configured_theme::configured_theme;

    #[test]
    fn severity_states_share_the_severity_colours() {
        let theme = configured_theme(&Palette::default());
        assert_eq!(IndicatorState::Failed.color(&theme), danger_color(&theme));
        assert_eq!(IndicatorState::Critical.color(&theme), danger_color(&theme));
        assert_eq!(IndicatorState::Warning.color(&theme), warning_color(&theme));
        assert_eq!(
            IndicatorState::Off.color(&theme),
            secondary_text_color(&theme)
        );
        assert_eq!(IndicatorState::Live.color(&theme), primary_color(&theme));
    }

    #[test]
    fn each_state_text_style_uses_its_colour() {
        let theme = configured_theme(&Palette::default());
        [
            IndicatorState::Failed,
            IndicatorState::Off,
            IndicatorState::Idle,
            IndicatorState::Live,
            IndicatorState::Warning,
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
            IndicatorState::Failed,
            IndicatorState::Off,
            IndicatorState::Idle,
            IndicatorState::Pending,
            IndicatorState::Warning,
            IndicatorState::On,
        ]
        .map(IndicatorState::weight);
        assert!(
            weights
                .iter()
                .enumerate()
                .all(|(index, weight)| !weights[index + 1..].contains(weight))
        );
        assert_eq!(
            IndicatorState::Critical.weight(),
            IndicatorState::On.weight()
        );
    }
}
