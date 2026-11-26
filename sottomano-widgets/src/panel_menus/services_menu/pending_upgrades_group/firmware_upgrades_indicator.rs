use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::ActionTint,
    host_machine_identity::{
        fwupd_firmware_upgrades::{FirmwareReleaseDate, FirmwareUpgrade, ReleaseSummary, Urgency},
        udev_hardware_identity::HardwareText,
    },
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::ReadoutBadge,
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
        terminal_command::TerminalCommand,
    },
    hover_cards::hover_card_body::{Section, headed_table},
    panel_menus::services_menu::pending_upgrades_group::{
        PendingUpgradesGroup, PendingUpgradesInteraction,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{
            NotificationBody, NotificationProgress, OngoingToastActionButton, StateToast,
        },
        value_cells::ValueCell,
    },
};

const FIRMWARE_UPGRADES_GLYPH: Phosphor = Phosphor::Circuitry(Weight::Regular);

pub const LABEL: &str = "Firmware upgrades";

impl PendingUpgradesGroup {
    pub(super) fn firmware_upgrade_indicator<'view>(
        &'view self,
        upgrades: &'view [FirmwareUpgrade],
        config: &Config,
    ) -> GroupIndicator<'view> {
        let install = PendingUpgradesInteraction::OpenTerminal(TerminalCommand::running(
            config,
            ["fwupdmgr", "update"].map(String::from),
        ));
        firmware_upgrades_indicator(upgrades)
            .on_press("run fwupdmgr update in a terminal", install.clone())
            .ongoing_toast_action_buttons([OngoingToastActionButton {
                button_label: "Install".to_owned(),
                tint: ActionTint::Proceed,
                on_button_pressed: install,
            }])
            .with_hover_card(Box::new(move |_| {
                self.firmware
                    .sections_described_by(|snapshot| firmware_upgrades_hover_card(snapshot))
            }))
    }
}

pub(super) fn firmware_upgrades_panel_visibility(upgrades: &[FirmwareUpgrade]) -> PanelVisibility {
    if upgrades.is_empty() {
        PanelVisibility::Hidden
    } else {
        PanelVisibility::Shown
    }
}

pub(super) fn firmware_upgrades_state(upgrades: &[FirmwareUpgrade]) -> IndicatorState {
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

pub(super) fn firmware_upgrades_hover_card(upgrades: &[FirmwareUpgrade]) -> Vec<Section> {
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

pub(super) fn firmware_upgrades_indicator(upgrades: &[FirmwareUpgrade]) -> GroupIndicator<'_> {
    let base = GroupIndicator::glyph_only(
        LABEL,
        Phosphor::Circuitry(Weight::Regular).into(),
        firmware_upgrades_state(upgrades),
    )
    .with_hover_card(Box::new(move |_| firmware_upgrades_hover_card(upgrades)));
    let badge = match firmware_upgrades_badge(upgrades) {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    base.with_badge(badge)
        .with_state_toast(StateToast::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "Firmware upgrade available",
            body: NotificationBody::Bodiless,
            progress: NotificationProgress::Unmeasured,
        })
        .with_panel_visibility(firmware_upgrades_panel_visibility(upgrades))
}

impl From<&FirmwareUpgrade> for TableRecord<6> {
    fn from(firmware_upgrade: &FirmwareUpgrade) -> Self {
        TableRecord::with_cells(
            firmware_upgrade.device.clone(),
            [
                match &firmware_upgrade.vendor {
                    HardwareText::Reported(vendor) => ValueCell::from(vendor.clone()),
                    HardwareText::Unreported => ValueCell::Blank,
                },
                firmware_upgrade.current_version.clone().into(),
                firmware_upgrade.offered_version.clone().into(),
                match firmware_upgrade.offered_released {
                    FirmwareReleaseDate::Dated(at) => ValueCell::Time {
                        at,
                        format: LocalizedTimeFormat::Date,
                    },
                    FirmwareReleaseDate::Undated => ValueCell::Blank,
                },
                match firmware_upgrade.urgency {
                    Urgency::Unspecified => ValueCell::Blank,
                    stated_urgency => stated_urgency.title().into(),
                },
                match &firmware_upgrade.summary {
                    ReleaseSummary::Summarized(summary) => ValueCell::from(summary.clone()),
                    ReleaseSummary::Unsummarized => ValueCell::Blank,
                },
            ],
        )
        .level(firmware_upgrade.severity())
    }
}

impl PendingUpgradesGroup {
    pub(super) fn firmware_upgrades_listing<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorListing<'view> {
        self.firmware.indicator_listing(
            |upgrades| match firmware_upgrades_panel_visibility(upgrades) {
                PanelVisibility::Shown => vec![self.firmware_upgrade_indicator(upgrades, config)],
                PanelVisibility::Hidden => Vec::new(),
            },
            FIRMWARE_UPGRADES_GLYPH.into(),
        )
    }
}
