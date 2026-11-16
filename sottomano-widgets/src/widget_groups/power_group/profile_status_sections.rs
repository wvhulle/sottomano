use iced::alignment::Horizontal;
use sottomano_desktop_sources::battery_power_profiles::power_profiles_daemon::{
    ProfileHold, ProfileStatus,
};

use crate::{
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        value_cells::LabeledValue,
    },
};

const HOLD_HEADINGS: [ColumnHeading; 2] = [
    ColumnHeading::titled("Profile"),
    ColumnHeading::titled("Reason").align_x(Horizontal::Left),
];

impl From<&ProfileHold> for TableRecord<2> {
    fn from(profile_hold: &ProfileHold) -> Self {
        TableRecord::with_cells(
            profile_hold.application_id.clone(),
            [
                profile_hold.profile.title().into(),
                profile_hold.reason.clone().into(),
            ],
        )
    }
}

pub fn profile_status_sections(profile_status: &ProfileStatus) -> Vec<Section> {
    let held_by = (!profile_status.holds.is_empty()).then(|| {
        Section::new(
            SectionTitle::captioned(
                "Held by",
                "Apps holding a profile until they finish; the slider follows them.",
            ),
            headed_table(HOLD_HEADINGS, profile_status.holds.iter()),
        )
    });
    let degraded = profile_status.degraded.as_ref().map(|reason| {
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
