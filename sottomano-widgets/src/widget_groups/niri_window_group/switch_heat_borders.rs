use iced::Task;
use sottomano_config::theme::palette::Palette;
use sottomano_state::{
    niri_rules::{HeatedWindow, NiriRulesFile, SwitchHeat},
    switch_heat_borders,
};

use crate::{
    shell_message::Message,
    widget_groups::niri_window_group::NiriWindowMessage,
    widgets::{
        phosphor::{Phosphor, Weight},
        toggle_control::{IconToggle, SwitchPresses},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReheatDue(bool);

impl ReheatDue {
    #[must_use]
    pub fn after(message: &Message) -> Self {
        Self(matches!(
            message,
            Message::Niri(_)
                | Message::SessionFocus(_)
                | Message::NiriWindow(_)
                | Message::NiriRulesLoaded(_)
                | Message::Clock(_)
        ))
    }
}

#[derive(Debug, Default)]
pub(super) struct SwitchHeatBorders {
    shown: bool,
}

impl SwitchHeatBorders {
    pub(super) fn load() -> Task<Message> {
        Task::perform(switch_heat_borders::load_shown(), |shown| {
            Message::NiriWindow(NiriWindowMessage::SwitchHeatLoaded(shown))
        })
    }

    pub(super) fn toggle(&self) -> IconToggle {
        IconToggle::new(
            "Switch heat",
            Phosphor::Fire(Weight::Regular),
            self.shown,
            SwitchPresses {
                turning_on: "tint each window's border warmer the more often you return to it, \
                             and ring the focused window",
                turning_off: "keep niri's own borders and focus ring",
            },
        )
    }

    pub(super) fn flip(&mut self) -> Task<Message> {
        self.shown = !self.shown;
        Task::future(switch_heat_borders::save_shown(self.shown)).discard()
    }

    pub(super) fn adopt_loaded(&mut self, shown: bool) {
        self.shown = shown;
    }

    pub(super) fn reheat(
        &self,
        due: ReheatDue,
        rules: &mut NiriRulesFile,
        palette: &Palette,
        heated_windows: impl FnOnce() -> Vec<HeatedWindow>,
    ) -> Task<Message> {
        let ReheatDue(true) = due else {
            return Task::none();
        };
        let switch_heat = if self.shown {
            SwitchHeat::Shown {
                focus_ring_color: format!("{:x}", palette.focus.to_rgba8()),
                neutral_border_color: format!("{:x}", palette.surface.to_rgba8()),
                windows: heated_windows(),
            }
        } else {
            SwitchHeat::Hidden
        };
        rules
            .reheat(switch_heat)
            .map_or_else(Task::none, |written| Task::future(written).discard())
    }
}
