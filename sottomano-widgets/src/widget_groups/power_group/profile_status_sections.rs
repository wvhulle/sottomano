use iced::alignment::Horizontal;
use sottomano_desktop_sources::battery_power_profiles::power_profiles_daemon::{
    ProfileHold, ProfileStatus,
};

use crate::{
    clock::Clock,
    hover_cards::hover_card_body::{
        ReadingSections, Section, SectionTitle, dictionary_table, headed_table,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        value_cells::LabeledValue,
    },
};

const HOLD_HEADINGS: [ColumnHeading; 2] = [
    ColumnHeading::titled("Profile"),
    ColumnHeading::titled("Reason").align_x(Horizontal::Left),
];

impl IntoTableRecord<2> for &ProfileHold {
    fn into_table_record(self) -> TableRecord<2> {
        TableRecord::new(
            self.application_id.clone(),
            [self.profile.title().into(), self.reason.clone().into()],
        )
    }
}

impl ReadingSections for ProfileStatus {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let held_by = (!self.holds.is_empty()).then(|| {
            Section::new(
                SectionTitle::captioned(
                    "Held by",
                    "Apps holding a profile until they finish; the slider follows them.",
                ),
                headed_table(HOLD_HEADINGS, self.holds.iter()),
            )
        });
        let degraded = self.degraded.as_ref().map(|reason| {
            Section::new(
                SectionTitle::captioned(
                    "Performance degraded",
                    "The daemon is throttling performance until this clears.",
                ),
                dictionary_table([LabeledValue::new("Reason", reason.clone())]),
            )
        });
        [held_by, degraded].into_iter().flatten().collect()
    }
}
