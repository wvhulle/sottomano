use iced::alignment::Horizontal;
use vetrata_sources::power_profile::{ProfileHold, ProfileStatus};

use crate::widgets::tooltip_body::{
    ColumnHeading, KeyValue, ReadingSections, Section, TableRecord, TableRow,
};

const HOLD_HEADINGS: [ColumnHeading; 2] = [
    ColumnHeading::new("Profile"),
    ColumnHeading::new("Reason").align_x(Horizontal::Left),
];

impl TableRow<2> for ProfileHold {
    fn table_record(&self) -> TableRecord<2> {
        TableRecord::new(
            self.application_id.clone(),
            [
                Some(self.profile.title().into()),
                Some(self.reason.clone().into()),
            ],
        )
    }
}

impl ReadingSections for ProfileStatus {
    fn sections(&self) -> Vec<Section> {
        let held_by = (!self.holds.is_empty()).then(|| {
            Section::column_table(
                HOLD_HEADINGS,
                self.holds.iter().map(ProfileHold::table_record),
            )
            .titled((
                "Held by",
                "Apps holding a profile until they finish; the slider follows them.",
            ))
        });
        let degraded = self.degraded.as_ref().map(|reason| {
            Section::dictionary([KeyValue::new("Reason", reason.clone())]).titled((
                "Performance degraded",
                "The daemon is throttling performance until this clears.",
            ))
        });
        [held_by, degraded].into_iter().flatten().collect()
    }
}
