use std::borrow::Cow;

use iced::{Subscription, Task};

use crate::{
    group_items::reading_row::{ReadingRow, reading_hover_card},
    hover_cards::hover_card_body::dictionary_table,
    widget_groups::widget_group_identity::WidgetGroupFamily,
    widgets::phosphor::Weight,
};

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
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        terminal_command::TerminalCommand,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
    widgets::{
        icon_glyph::IconGlyph, localized_quantities::Quantity, phosphor::Phosphor,
        subscribed_snapshot::SubscribedSnapshot, value_cells::LabeledValue,
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
    firmware: SubscribedSnapshot<Vec<FirmwareUpgrade>>,
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
    pub fn hover_card_for(&self, target: PendingUpgradesTarget, clock: Clock) -> Vec<Section> {
        match target {
            PendingUpgradesTarget::Firmware => self
                .firmware
                .sections_described_by(|snapshot| reading_hover_card(snapshot, clock)),
            PendingUpgradesTarget::Summary => self.firmware.hover_card_with(self.hover_card(clock)),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a, PendingUpgradesInteraction> {
        let locale = config.calendar.locale;
        let has_categories = self.category_states(clock).iter().any(Option::is_some);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PendingUpgrades,
            &self.expansion,
            PendingUpgradesInteraction::Expansion,
            [
                has_categories.then(|| {
                    GroupItem::reading(
                        ReadingRow::evaluated(self, clock, locale),
                        PendingUpgradesTarget::Summary,
                    )
                }),
                self.firmware_upgrade_item(config, clock),
                self.firmware.failure().map(|failure| {
                    GroupItem::reading(
                        failure.reading_row(IconGlyph::default()),
                        PendingUpgradesTarget::Firmware,
                    )
                }),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn category_badges(&self) -> [Option<usize>; 1] {
        [self
            .firmware
            .visible_reading(GroupItemReading::panel_visibility)
            .and_then(GroupItemReading::badge)]
    }

    fn category_states(&self, clock: Clock) -> [Option<ReadingDisplayState>; 1] {
        [self.firmware.shown_state(
            |snapshot| snapshot.state(clock),
            GroupItemReading::panel_visibility,
            clock,
        )]
    }
}

impl GroupItemReading for PendingUpgradesGroup {
    fn name(&self) -> Cow<'static, str> {
        "Available upgrades".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowCircleUp(Weight::Regular).into()
    }

    fn badge(&self) -> Option<usize> {
        let total: usize = self.category_badges().into_iter().flatten().sum();
        (total > 0).then_some(total)
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        self.category_states(clock)
            .into_iter()
            .flatten()
            .map(ReadingDisplayState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Count",
            dictionary_table(
                self.firmware
                    .visible_reading(GroupItemReading::panel_visibility)
                    .map(|upgrades| {
                        LabeledValue::new(
                            firmware_upgrade_list::LABEL,
                            Quantity::count(upgrades.len()),
                        )
                    }),
            ),
        )]
    }
}
