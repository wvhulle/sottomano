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
    hover_cards::HoverTarget,
    widget_groups::terminal_command::TerminalCommand,
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
        icon_glyph::IconGlyph,
        latest_snapshot::LatestSnapshot,
        localized_quantities::Quantity,
        phosphor::Phosphor,
        reading_display_state::ReadingDisplayState,
        value_cells::LabeledValue,
        widget_group::{
            WidgetGroup, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
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
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PendingUpgradesMessage {
    Firmware(SnapshotDelivery<Vec<FirmwareUpgrade>>),
    OpenTerminal(TerminalCommand),
    Expansion(WidgetGroupExpansionMessage),
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
    pub fn hover_card_for(&self, target: PendingUpgradesTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            PendingUpgradesTarget::Firmware => self.firmware.hover_card(clock),
            PendingUpgradesTarget::Summary => self.firmware.hover_card_with(self.hover_card(clock)),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, PendingUpgradesMessage> {
        let has_categories = self.categories().iter().any(Option::is_some);
        Self::expandable_group(
            &self.expansion,
            PendingUpgradesMessage::Expansion,
            [
                has_categories.then(|| {
                    GroupItem::reading(
                        self as &dyn GroupItemReading,
                        PendingUpgradesTarget::Summary,
                    )
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

    fn categories(&self) -> [Option<&dyn GroupItemReading>; 1] {
        [self.firmware.display()]
    }
}

impl GroupItemReading for PendingUpgradesGroup {
    fn name(&self) -> &'static str {
        "Pending upgrades"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowCircleUp(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        let total: u32 = self
            .categories()
            .into_iter()
            .flatten()
            .filter_map(GroupItemReading::badge)
            .sum();
        (total > 0).then_some(total)
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        self.categories()
            .into_iter()
            .flatten()
            .map(|category| category.state(clock).severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(self.firmware.visible_reading().map(|upgrades| {
            LabeledValue::new(
                firmware_upgrade_list::LABEL,
                Quantity::count(upgrades.len()),
            )
        }))
    }
}
