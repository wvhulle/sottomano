use iced::{Subscription, Task};

use crate::{widget_groups::widget_group_identity::WidgetGroupFamily, widgets::phosphor::Weight};

mod firmware_upgrade_list;

use sottomano_desktop_sources::{
    SnapshotDelivery, host_machine_identity::fwupd_firmware_upgrades::FirmwareUpgrade,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        terminal_command::TerminalCommand,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
    widgets::{
        icon_glyph::IconGlyph, latest_snapshot::LatestSnapshot, localized_quantities::Quantity,
        phosphor::Phosphor, value_cells::LabeledValue,
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

#[derive(Debug)]
pub enum PendingUpgradesMessage {
    Firmware(SnapshotDelivery<Vec<FirmwareUpgrade>>),
    Interaction(PendingUpgradesInteraction),
}

#[derive(Debug, Clone)]
pub enum PendingUpgradesInteraction {
    OpenTerminal(TerminalCommand),
    Expansion(WidgetGroupExpansionMessage),
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
        upgrades_message: PendingUpgradesMessage,
        clock: Clock,
    ) -> Task<PendingUpgradesMessage> {
        match upgrades_message {
            PendingUpgradesMessage::Firmware(upgrades) => {
                self.firmware.receive(upgrades, clock);
                Task::none()
            }
            PendingUpgradesMessage::Interaction(PendingUpgradesInteraction::OpenTerminal(
                terminal,
            )) => terminal.open(),
            PendingUpgradesMessage::Interaction(PendingUpgradesInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
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
    ) -> WidgetGroupContent<'a, PendingUpgradesInteraction> {
        let has_categories = self.categories().iter().any(Option::is_some);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PendingUpgrades,
            &self.expansion,
            PendingUpgradesInteraction::Expansion,
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
