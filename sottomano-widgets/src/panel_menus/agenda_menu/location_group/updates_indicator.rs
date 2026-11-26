use situo_client::{UpdateStatus, UpdateTrigger};
use sottomano_desktop_sources::weather::situo_location_fix::Location;

use super::LocationGroup;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

pub struct UpdatesIndicator<'view> {
    pub location: &'view Location,
}

impl<'view> UpdatesIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let update_status = self.location.update_status;
        let state = match update_status {
            UpdateStatus::Locating => IndicatorState::Pending,
            UpdateStatus::Idle => IndicatorState::Idle,
            UpdateStatus::BeaconDbUnreachable
            | UpdateStatus::BeaconDbRefused
            | UpdateStatus::BeaconDbAnswerInvalid
            | UpdateStatus::DaemonMalfunction => IndicatorState::Critical,
        };
        GroupIndicator::glyph_only(
            "Updates",
            Phosphor::ArrowsClockwise(Weight::Regular).into(),
            state,
        )
        .with_label(ReadoutLabel::Labeled(
            update_status_label(update_status).to_owned(),
        ))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let status_rows = [LabeledValue::new(
            "Last lookup",
            update_status_label(self.location.update_status),
        )];
        let schedule_rows = [
            LabeledValue::new(
                "Fallback interval",
                ValueCell::duration(self.location.fallback_interval()),
            ),
            LabeledValue::new(
                "Cell check interval",
                ValueCell::duration(self.location.cell_check_interval()),
            ),
        ];
        let trigger_rows = ALL_UPDATE_TRIGGERS.map(|update_trigger| {
            let watch_state = if self
                .location
                .update_schedule
                .triggers
                .contains(&update_trigger)
            {
                "watched"
            } else {
                "not watched"
            };
            LabeledValue::new(update_trigger_label(update_trigger), watch_state)
        });
        vec![
            Section::new("Status", dictionary_table(status_rows)),
            Section::new(
                SectionTitle::captioned(
                    "Schedule",
                    "When situo looks up again; the fallback interval applies when nothing else \
                     triggers.",
                ),
                dictionary_table(schedule_rows.into_iter().chain(trigger_rows)),
            ),
        ]
    }
}

const fn update_status_label(update_status: UpdateStatus) -> &'static str {
    match update_status {
        UpdateStatus::Locating => "Locating",
        UpdateStatus::Idle => "Done",
        UpdateStatus::BeaconDbUnreachable => "BeaconDB unreachable",
        UpdateStatus::BeaconDbRefused => "Refused by BeaconDB",
        UpdateStatus::BeaconDbAnswerInvalid => "Unusable answer",
        UpdateStatus::DaemonMalfunction => "Daemon fault",
    }
}

const fn update_trigger_label(update_trigger: UpdateTrigger) -> &'static str {
    match update_trigger {
        UpdateTrigger::ServingCellChange => "Serving cell change",
        UpdateTrigger::WifiNeighbourhoodChange => "Wi-Fi neighbourhood change",
        UpdateTrigger::NetworkReconnect => "Network reconnect",
        UpdateTrigger::ModemHotplug => "Modem added or removed",
    }
}

const ALL_UPDATE_TRIGGERS: [UpdateTrigger; 4] = [
    UpdateTrigger::ServingCellChange,
    UpdateTrigger::WifiNeighbourhoodChange,
    UpdateTrigger::NetworkReconnect,
    UpdateTrigger::ModemHotplug,
];

impl LocationGroup {
    pub(super) fn updates_row<'view>(
        &'view self,
        location: &'view Location,
    ) -> GroupIndicator<'view> {
        UpdatesIndicator { location }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.location
                    .hover_card_with(UpdatesIndicator { location }.hover_card())
            }))
    }
}
