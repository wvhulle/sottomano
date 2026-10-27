use chrono::Locale;
use sottomano_sources::gamepad_controller_input::player_indicator_leds::LitPlayer;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct PlayerNumberReading<'a> {
    pub lit_player: &'a LitPlayer,
}

impl PlayerNumberReading<'_> {
    fn led_pattern(&self) -> String {
        self.lit_player
            .leds_lit
            .iter()
            .map(|lit| if *lit { '●' } else { '○' })
            .collect()
    }
}

impl BarWidget for PlayerNumberReading<'_> {
    fn name(&self) -> &'static str {
        "Player number"
    }

    fn icon(&self) -> Glyph {
        Phosphor::FlagBanner(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.lit_player.number.to_string())
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::Idle
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary([
                KeyValue::new("Player", self.lit_player.number.to_string()),
                KeyValue::new("Lights", self.led_pattern()),
            ]),
            Section::note(
                "Numbered by the kernel",
                Some(
                    "Assigned in the order the controllers connected, and shown on the \
                     controller's lights. Games may number players differently, for example by \
                     who presses a button first."
                        .to_owned(),
                ),
            ),
        ])
    }
}
