use iced::{Subscription, Task};

use crate::widgets::phosphor::Weight;

mod firmware_upgrade_list;

use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery, host_machine_identity::fwupd_firmware_upgrades::FirmwareUpgrade,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::terminal_command::TerminalCommand,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        localized_quantities::Quantity,
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, tooltip_source_delivery::Tooltip},
        value_cells::LabeledValue,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, WidgetGroup},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingUpgradesTarget {
    Summary,
    Firmware,
}

impl From<PendingUpgradesTarget> for HoverTarget {
    fn from(target: PendingUpgradesTarget) -> Self {
        HoverTarget::PendingUpgrades(target)
    }
}

#[derive(Default)]
pub struct PendingUpgradesGroup {
    firmware: LatestSnapshot<Vec<FirmwareUpgrade>>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PendingUpgradesMessage {
    Firmware(SnapshotDelivery<Vec<FirmwareUpgrade>>),
    OpenTerminal(TerminalCommand),
    Expansion(ExpansionMessage),
}

impl WidgetGroup for PendingUpgradesGroup {
    const TITLE: &'static str = "Pending upgrades";
}

impl PendingUpgradesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<PendingUpgradesMessage> {
        snapshot_subscription::<Vec<FirmwareUpgrade>>(buses.clone())
            .map(PendingUpgradesMessage::Firmware)
    }

    pub fn update(
        &mut self,
        msg: PendingUpgradesMessage,
        clock: Clock,
    ) -> Task<PendingUpgradesMessage> {
        match msg {
            PendingUpgradesMessage::Firmware(upgrades) => {
                self.firmware.receive(upgrades, clock);
            }
            PendingUpgradesMessage::OpenTerminal(terminal) => return terminal.open(),
            PendingUpgradesMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: PendingUpgradesTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            PendingUpgradesTarget::Firmware => self.firmware.tooltip(clock),
            PendingUpgradesTarget::Summary => self.firmware.tooltip_with(self.tooltip(clock)),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, PendingUpgradesMessage> {
        let has_categories = self.categories().iter().any(Option::is_some);
        Self::expandable_group(
            &self.expansion,
            PendingUpgradesMessage::Expansion,
            [
                has_categories.then(|| {
                    GroupItem::reading(self as &dyn BarWidget, PendingUpgradesTarget::Summary)
                }),
                self.firmware_upgrade_item(config),
                self.firmware
                    .failure()
                    .map(|failure| GroupItem::reading(failure, PendingUpgradesTarget::Firmware)),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn categories(&self) -> [Option<&dyn BarWidget>; 1] {
        [self.firmware.display()]
    }
}

impl BarWidget for PendingUpgradesGroup {
    fn name(&self) -> &'static str {
        "Pending upgrades"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ArrowCircleUp(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        let total: u32 = self
            .categories()
            .into_iter()
            .flatten()
            .filter_map(BarWidget::badge)
            .sum();
        (total > 0).then_some(total)
    }

    fn state(&self, clock: Clock) -> WidgetState {
        self.categories()
            .into_iter()
            .flatten()
            .map(|category| category.state(clock).severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(self.firmware.visible_reading().map(|upgrades| {
            LabeledValue::new(
                firmware_upgrade_list::LABEL,
                Quantity::count(upgrades.len()),
            )
        }))
    }
}
