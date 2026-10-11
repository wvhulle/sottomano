use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::fwupd::{FirmwareUpgrade, Urgency},
    },
    widget_groups::{
        pending_upgrades_group::{PendingUpgradesMessage, PendingUpgradesTarget},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        group_item::GroupItem,
        localized_time::TimePart,
        phosphor::Phosphor,
        reading_alert::ReadingAlertActionButton,
        tooltip_body::{Cell, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

pub const LABEL: &str = "Firmware upgrades";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareUpgradeList {
    upgrades: Vec<FirmwareUpgrade>,
}

impl FirmwareUpgradeList {
    pub fn from_upgrades(upgrades: Vec<FirmwareUpgrade>) -> Option<Self> {
        (!upgrades.is_empty()).then_some(Self { upgrades })
    }

    pub fn count(&self) -> usize {
        self.upgrades.len()
    }

    pub fn group_item(&self, config: &Config) -> GroupItem<'_, PendingUpgradesMessage> {
        let install = PendingUpgradesMessage::OpenTerminal(TerminalCommand::running(
            config,
            ["fwupdmgr", "update"].map(String::from),
        ));
        GroupItem::reading(self, PendingUpgradesTarget::Firmware)
            .on_press("run fwupdmgr update in a terminal", install.clone())
            .alert_action_buttons([ReadingAlertActionButton {
                button_label: "Install".to_owned(),
                on_button_pressed: install,
            }])
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

impl BarWidget for FirmwareUpgradeList {
    fn name(&self) -> &'static str {
        "Firmware upgrades"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Circuitry(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.upgrades.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.upgrades
            .iter()
            .map(FirmwareUpgrade::severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn reading_alert_threshold(&self) -> ReadingAlertThreshold {
        ReadingAlertThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "Firmware upgrade available",
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::column_table(
            LABEL,
            [
                "Vendor",
                "Installed",
                "Offered",
                "Released",
                "Urgency",
                "Summary",
            ],
            self.upgrades.iter().map(FirmwareUpgrade::table_record),
        )])
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
