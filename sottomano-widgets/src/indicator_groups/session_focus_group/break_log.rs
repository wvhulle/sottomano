use sottomano_desktop_sources::login_session_presence::focus_time_ledger::Accounting;

use crate::{
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

pub struct BreakLog<'a> {
    pub accounting: Accounting<'a>,
}

impl BreakLog<'_> {
    pub fn focus_settings(&self) -> Section {
        let config = self.accounting.config;
        Section::new(
            SectionTitle::captioned(
                "Settings".to_owned(),
                "Turning the screen off or suspending always counts as a break, however short. \
                 Alerts count from the end of the last break."
                    .to_owned(),
            ),
            dictionary_table([
                LabeledValue::new(
                    "Idle after no input for",
                    ValueCell::duration(config.idle_detection_delay),
                ),
                LabeledValue::new(
                    "Idle pause becomes a break after",
                    ValueCell::duration(config.idle_after),
                ),
                LabeledValue::new(
                    "Warning after active for",
                    ValueCell::duration(config.session_warning),
                ),
                LabeledValue::new(
                    "Critical after active for",
                    ValueCell::duration(config.session_critical),
                ),
                LabeledValue::new("Snooze lasts", ValueCell::duration(config.warning_snooze)),
            ]),
        )
    }
}
