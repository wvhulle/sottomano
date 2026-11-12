use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::ActionTint,
    host_machine_identity::fwupd_firmware_upgrades::FirmwareUpgrade,
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::{GroupItemReading, OngoingToastThreshold, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::{
        pending_upgrades_group::{
            PendingUpgradesGroup, PendingUpgradesInteraction, PendingUpgradesTarget,
        },
        terminal_command::TerminalCommand,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::OngoingToastActionButton,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

pub const LABEL: &str = "Firmware upgrades";

impl PendingUpgradesGroup {
    pub(super) fn firmware_upgrade_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, PendingUpgradesInteraction>> {
        let upgrades = self.firmware.visible_reading()?;
        let install = PendingUpgradesInteraction::OpenTerminal(TerminalCommand::running(
            config,
            ["fwupdmgr", "update"].map(String::from),
        ));
        Some(
            GroupItem::reading(upgrades, PendingUpgradesTarget::Firmware)
                .on_press("run fwupdmgr update in a terminal", install.clone())
                .ongoing_toast_action_buttons([OngoingToastActionButton {
                    button_label: "Install".to_owned(),
                    tint: ActionTint::Proceed,
                    on_button_pressed: install,
                }]),
        )
    }
}

impl GroupItemReading for Vec<FirmwareUpgrade> {
    fn name(&self) -> &'static str {
        "Firmware upgrades"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Circuitry(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.len()).ok()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.iter()
            .map(FirmwareUpgrade::severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "Firmware upgrade available",
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Vendor").align_x(Horizontal::Left),
                ColumnHeading::titled("Installed"),
                ColumnHeading::titled("Offered"),
                ColumnHeading::titled("Released"),
                ColumnHeading::titled("Urgency").align_x(Horizontal::Left),
                ColumnHeading::titled("Summary").align_x(Horizontal::Left),
            ],
            self.iter(),
        )
        .titled(LABEL)])
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.is_empty() {
            PanelVisibility::Hidden
        } else {
            PanelVisibility::Shown
        }
    }
}

impl IntoTableRecord<6> for &FirmwareUpgrade {
    fn into_table_record(self) -> TableRecord<6> {
        TableRecord::new(
            self.device.clone(),
            [
                self.vendor
                    .clone()
                    .map_or(ValueCell::Blank, ValueCell::from),
                self.current_version.clone().into(),
                self.offered_version.clone().into(),
                self.offered_released
                    .map_or(ValueCell::Blank, |at| ValueCell::Time {
                        at,
                        format: LocalizedTimeFormat::Date,
                    }),
                self.urgency
                    .map_or(ValueCell::Blank, |urgency| urgency.title().into()),
                self.summary
                    .clone()
                    .map_or(ValueCell::Blank, ValueCell::from),
            ],
        )
        .level(self.severity())
    }
}
