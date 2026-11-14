use corredo::theming::{
    danger_color, primary_color, secondary_text_color, text_color, warning_color,
};
use iced::{Color, Theme, widget::text};
use sottomano_desktop_sources::{
    media_playback_audio::mpris_players::{LoopMode, QueueOrder},
    network_connectivity::{
        network_manager_proxies::{DeviceLinkState, RadioPower},
        tailscale_exit_nodes::ExitNodeLanAccess,
        tailscale_status::{ExitNodeOffer, ExitNodeUse},
    },
};
use sottomano_toml_config::severity::Severity;

use crate::widgets::phosphor::Weight;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadingDisplayState {
    Failed,
    Off,
    Offline,
    #[default]
    Idle,
    Pending,
    Warning,
    On,
    Live,
    Critical,
}

impl ReadingDisplayState {
    #[must_use]
    pub const fn weight(self) -> Weight {
        match self {
            ReadingDisplayState::Failed => Weight::Thin,
            ReadingDisplayState::Off => Weight::Light,
            ReadingDisplayState::Offline | ReadingDisplayState::Pending => Weight::Duotone,
            ReadingDisplayState::Idle => Weight::Regular,
            ReadingDisplayState::Warning => Weight::Bold,
            ReadingDisplayState::On | ReadingDisplayState::Live | ReadingDisplayState::Critical => {
                Weight::Fill
            }
        }
    }

    #[must_use]
    pub fn color(self, theme: &Theme) -> Color {
        match self {
            ReadingDisplayState::Failed | ReadingDisplayState::Critical => danger_color(theme),
            ReadingDisplayState::Off => secondary_text_color(theme),
            ReadingDisplayState::Idle | ReadingDisplayState::On => text_color(theme),
            ReadingDisplayState::Live => primary_color(theme),
            ReadingDisplayState::Offline
            | ReadingDisplayState::Pending
            | ReadingDisplayState::Warning => warning_color(theme),
        }
    }

    #[must_use]
    pub const fn text_style(self) -> fn(&Theme) -> text::Style {
        match self {
            ReadingDisplayState::Failed | ReadingDisplayState::Critical => text::danger,
            ReadingDisplayState::Off => text::secondary,
            ReadingDisplayState::Idle | ReadingDisplayState::On => text::base,
            ReadingDisplayState::Live => text::primary,
            ReadingDisplayState::Offline
            | ReadingDisplayState::Pending
            | ReadingDisplayState::Warning => text::warning,
        }
    }

    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            ReadingDisplayState::Failed | ReadingDisplayState::Critical => Severity::Critical,
            ReadingDisplayState::Warning => Severity::Warning,
            ReadingDisplayState::Off
            | ReadingDisplayState::Offline
            | ReadingDisplayState::Idle
            | ReadingDisplayState::Pending
            | ReadingDisplayState::On
            | ReadingDisplayState::Live => Severity::Normal,
        }
    }
}

impl From<DeviceLinkState> for ReadingDisplayState {
    fn from(link_state: DeviceLinkState) -> Self {
        match link_state {
            DeviceLinkState::Connected => ReadingDisplayState::On,
            DeviceLinkState::Switching => ReadingDisplayState::Pending,
            DeviceLinkState::Disconnected => ReadingDisplayState::Off,
        }
    }
}

impl From<RadioPower> for ReadingDisplayState {
    fn from(radio_power: RadioPower) -> Self {
        match radio_power {
            RadioPower::On => ReadingDisplayState::On,
            RadioPower::Off => ReadingDisplayState::Off,
        }
    }
}

impl From<ExitNodeUse> for ReadingDisplayState {
    fn from(exit_node_use: ExitNodeUse) -> Self {
        match exit_node_use {
            ExitNodeUse::InUse => ReadingDisplayState::On,
            ExitNodeUse::Unused => ReadingDisplayState::Off,
        }
    }
}

impl From<ExitNodeOffer> for ReadingDisplayState {
    fn from(exit_offer: ExitNodeOffer) -> Self {
        match exit_offer {
            ExitNodeOffer::NotOffered => ReadingDisplayState::Off,
            ExitNodeOffer::AwaitingApproval => ReadingDisplayState::Pending,
            ExitNodeOffer::Approved => ReadingDisplayState::On,
        }
    }
}

impl From<ExitNodeLanAccess> for ReadingDisplayState {
    fn from(lan_access: ExitNodeLanAccess) -> Self {
        match lan_access {
            ExitNodeLanAccess::Allowed => ReadingDisplayState::On,
            ExitNodeLanAccess::Blocked => ReadingDisplayState::Off,
        }
    }
}

impl From<QueueOrder> for ReadingDisplayState {
    fn from(queue_order: QueueOrder) -> Self {
        match queue_order {
            QueueOrder::Shuffled => ReadingDisplayState::On,
            QueueOrder::InOrder => ReadingDisplayState::Off,
        }
    }
}

impl From<LoopMode> for ReadingDisplayState {
    fn from(loop_mode: LoopMode) -> Self {
        match loop_mode {
            LoopMode::Off => ReadingDisplayState::Off,
            LoopMode::Track | LoopMode::Playlist => ReadingDisplayState::On,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReadingStateTint {
    pub state: ReadingDisplayState,
    pub opacity: f32,
}

impl ReadingStateTint {
    #[must_use]
    pub const fn opaque(state: ReadingDisplayState) -> Self {
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

impl From<Severity> for ReadingDisplayState {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Low => ReadingDisplayState::Off,
            Severity::Normal => ReadingDisplayState::Idle,
            Severity::Warning => ReadingDisplayState::Warning,
            Severity::Critical => ReadingDisplayState::Critical,
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
        assert_eq!(
            ReadingDisplayState::Failed.color(&theme),
            danger_color(&theme)
        );
        assert_eq!(
            ReadingDisplayState::Critical.color(&theme),
            danger_color(&theme)
        );
        assert_eq!(
            ReadingDisplayState::Warning.color(&theme),
            warning_color(&theme)
        );
        assert_eq!(
            ReadingDisplayState::Offline.color(&theme),
            warning_color(&theme)
        );
        assert_eq!(
            ReadingDisplayState::Off.color(&theme),
            secondary_text_color(&theme)
        );
        assert_eq!(
            ReadingDisplayState::Live.color(&theme),
            primary_color(&theme)
        );
    }

    #[test]
    fn each_state_text_style_uses_its_colour() {
        let theme = configured_theme(&Palette::default());
        [
            ReadingDisplayState::Failed,
            ReadingDisplayState::Off,
            ReadingDisplayState::Offline,
            ReadingDisplayState::Idle,
            ReadingDisplayState::Live,
            ReadingDisplayState::Warning,
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
            ReadingDisplayState::Failed,
            ReadingDisplayState::Off,
            ReadingDisplayState::Idle,
            ReadingDisplayState::Pending,
            ReadingDisplayState::Warning,
            ReadingDisplayState::On,
        ]
        .map(ReadingDisplayState::weight);
        assert!(
            weights
                .iter()
                .enumerate()
                .all(|(index, weight)| !weights[index + 1..].contains(weight))
        );
        assert_eq!(
            ReadingDisplayState::Offline.weight(),
            ReadingDisplayState::Pending.weight()
        );
        assert_eq!(
            ReadingDisplayState::Critical.weight(),
            ReadingDisplayState::On.weight()
        );
    }
}
