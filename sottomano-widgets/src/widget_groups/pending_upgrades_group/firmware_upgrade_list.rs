use chrono::Locale;
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
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, RowReadout},
    },
    hover_cards::hover_card_body::{Section, headed_table},
    widget_groups::{
        pending_upgrades_group::{
            PendingUpgradesGroup, PendingUpgradesInteraction, PendingUpgradesTarget,
        },
        terminal_command::TerminalCommand,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{
            OngoingToastActionButton, ReadingAlerts, StateToast, ToastBody, ToastProgress,
        },
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

pub const LABEL: &str = "Firmware upgrades";

impl PendingUpgradesGroup {
    pub(super) fn firmware_upgrade_item(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Option<GroupItem<'_, PendingUpgradesInteraction>> {
        let locale = config.calendar.locale;
        let upgrades = self
            .firmware
            .visible_reading(|upgrades| firmware_upgrades_panel_visibility(upgrades))?;
        let install = PendingUpgradesInteraction::OpenTerminal(TerminalCommand::running(
            config,
            ["fwupdmgr", "update"].map(String::from),
        ));
        Some(
            GroupItem::reading(
                firmware_upgrades_reading_row(upgrades, clock, locale),
                PendingUpgradesTarget::Firmware,
            )
            .on_press("run fwupdmgr update in a terminal", install.clone())
            .ongoing_toast_action_buttons([OngoingToastActionButton {
                button_label: "Install".to_owned(),
                tint: ActionTint::Proceed,
                on_button_pressed: install,
            }]),
        )
    }
}

pub(super) fn firmware_upgrades_panel_visibility(upgrades: &[FirmwareUpgrade]) -> PanelVisibility {
    if upgrades.is_empty() {
        PanelVisibility::Hidden
    } else {
        PanelVisibility::Shown
    }
}

pub(super) fn firmware_upgrades_state(upgrades: &[FirmwareUpgrade]) -> ReadingDisplayState {
    upgrades
        .iter()
        .map(FirmwareUpgrade::severity)
        .max()
        .unwrap_or_default()
        .into()
}

pub(super) fn firmware_upgrades_badge(upgrades: &[FirmwareUpgrade]) -> usize {
    upgrades.len()
}

pub(super) fn firmware_upgrades_hover_card(
    upgrades: &[FirmwareUpgrade],
    _clock: Clock,
) -> Vec<Section> {
    vec![Section::new(
        LABEL,
        headed_table(
            [
                ColumnHeading::titled("Vendor").align_x(Horizontal::Left),
                ColumnHeading::titled("Installed"),
                ColumnHeading::titled("Offered"),
                ColumnHeading::titled("Released"),
                ColumnHeading::titled("Urgency").align_x(Horizontal::Left),
                ColumnHeading::titled("Summary").align_x(Horizontal::Left),
            ],
            upgrades.iter(),
        ),
    )]
}

pub(super) fn firmware_upgrades_reading_row(
    upgrades: &[FirmwareUpgrade],
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let base = ReadingRow::glyph_only(
        LABEL,
        Phosphor::Circuitry(Weight::Regular).into(),
        firmware_upgrades_state(upgrades),
        Box::new(move |card_clock| firmware_upgrades_hover_card(upgrades, card_clock)),
    );
    let badge = match firmware_upgrades_badge(upgrades) {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    ReadingRow {
        readout: RowReadout {
            badge,
            ..base.readout
        },
        alerts: ReadingAlerts {
            state_toast: StateToast::NotifyFromSeverity {
                minimum_severity: Severity::Warning,
                summary_text: "Firmware upgrade available",
                body: ToastBody::Bodiless,
                progress: ToastProgress::Unmeasured,
            },
            ..base.alerts
        },
        panel_visibility: firmware_upgrades_panel_visibility(upgrades),
        ..base
    }
}

impl IntoTableRecord<6> for &FirmwareUpgrade {
    fn into_table_record(self) -> TableRecord<6> {
        TableRecord::with_cells(
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
