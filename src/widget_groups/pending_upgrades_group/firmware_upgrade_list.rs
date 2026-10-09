use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::fwupd::{FirmwareUpgrade, Urgency},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::{
        pending_upgrades_group::PendingUpgradesMessage, terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        group_item::GroupItem,
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, table_section},
        widget_state::WidgetState,
    },
};

const LABEL: &str = "Firmware upgrades";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareUpgradeList {
    upgrades: Vec<FirmwareUpgrade>,
}

impl FirmwareUpgradeList {
    pub fn from_upgrades(upgrades: Vec<FirmwareUpgrade>) -> Option<Self> {
        (!upgrades.is_empty()).then_some(Self { upgrades })
    }

    pub fn group_item(&self, config: &Config) -> GroupItem<'_, PendingUpgradesMessage> {
        let install = PendingUpgradesMessage::OpenTerminal(TerminalCommand::running(
            config,
            ["fwupdmgr", "update"].map(String::from),
        ));
        GroupItem::reading(self, HoverTarget::FirmwareUpgrades)
            .on_press("run fwupdmgr update in a terminal", install.clone())
            .alert_action_button("Install", install)
    }

    pub fn count_row(&self) -> Row {
        row(LABEL, self.upgrades.len().to_string())
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
        TooltipBody::Sections(vec![table_section(LABEL, self.upgrades.iter())])
    }
}

impl TableRow for &FirmwareUpgrade {
    const HEADINGS: &'static [&'static str] = &[
        "Vendor",
        "Installed",
        "Offered",
        "Released",
        "Urgency",
        "Summary",
    ];

    fn key(&self) -> Cell {
        self.device.clone().into()
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            self.vendor.clone().map_or(Cell::Missing, Cell::from),
            self.current_version.clone().into(),
            self.offered_version.clone().into(),
            self.offered_released
                .map_or(Cell::Missing, |at| Cell::Time {
                    at,
                    part: TimePart::Date,
                }),
            self.urgency
                .map_or(Cell::Missing, |urgency| urgency.title().into()),
            self.summary.clone().map_or(Cell::Missing, Cell::from),
        ]
    }

    fn level(&self) -> Severity {
        self.severity()
    }
}
