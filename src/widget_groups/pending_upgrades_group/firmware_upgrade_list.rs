use crate::{
    config::Config,
    data::{
        notifications::model::ActionTint,
        severity::Severity,
        source::fwupd::{FirmwareUpgrade, Urgency},
    },
    widget_groups::{
        pending_upgrades_group::{
            PendingUpgradesGroup, PendingUpgradesMessage, PendingUpgradesTarget,
        },
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional, OngoingToastThreshold},
        group_item::GroupItem,
        localized_time::TimePart,
        ongoing_toast::OngoingToastActionButton,
        phosphor::Phosphor,
        tooltip_body::{Cell, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

pub const LABEL: &str = "Firmware upgrades";

impl PendingUpgradesGroup {
    pub(super) fn firmware_upgrade_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, PendingUpgradesMessage>> {
        let upgrades = self.firmware.present_reading()?;
        let install = PendingUpgradesMessage::OpenTerminal(TerminalCommand::running(
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

impl Occasional for Vec<FirmwareUpgrade> {
    fn present(&self) -> bool {
        !self.is_empty()
    }
}

impl FirmwareUpgrade {
    fn severity(&self) -> Severity {
        match self.urgency {
            Some(Urgency::High | Urgency::Critical) => Severity::Critical,
            Some(Urgency::Low | Urgency::Medium) | None => Severity::Warning,
        }
    }
}

impl BarWidget for Vec<FirmwareUpgrade> {
    fn name(&self) -> &'static str {
        "Firmware upgrades"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Circuitry(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.len()).ok()
    }

    fn state(&self) -> WidgetState {
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

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                "Vendor",
                "Installed",
                "Offered",
                "Released",
                "Urgency",
                "Summary",
            ],
            self.iter().map(FirmwareUpgrade::table_record),
        )
        .titled(LABEL)])
    }
}

impl FirmwareUpgrade {
    fn table_record(&self) -> TableRecord<6> {
        TableRecord::new(
            self.device.clone(),
            [
                self.vendor.clone().map(Cell::from),
                Some(self.current_version.clone().into()),
                Some(self.offered_version.clone().into()),
                self.offered_released.map(|at| Cell::Time {
                    at,
                    part: TimePart::Date,
                }),
                self.urgency.map(|urgency| urgency.title().into()),
                self.summary.clone().map(Cell::from),
            ],
        )
        .level(self.severity())
    }
}
