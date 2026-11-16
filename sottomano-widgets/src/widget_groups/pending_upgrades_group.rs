use iced::{Subscription, Task};

use crate::{
    group_items::{
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, RowReadout},
    },
    hover_cards::hover_card_body::dictionary_table,
    widget_groups::widget_group_identity::WidgetGroupFamily,
    widgets::phosphor::Weight,
};

mod firmware_upgrade_list;

use firmware_upgrade_list::{
    firmware_upgrades_badge, firmware_upgrades_hover_card, firmware_upgrades_panel_visibility,
    firmware_upgrades_state,
};
use sottomano_desktop_sources::{
    SnapshotDelivery, host_machine_identity::fwupd_firmware_upgrades::FirmwareUpgrade,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{group_item::GroupItem, reading_display_state::ReadingDisplayState},
    hover_cards::hover_card_body::Section,
    widget_groups::{
        group_summary::GroupSummary,
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

#[derive(Default)]
pub struct PendingUpgradesGroup {
    firmware: SubscribedSnapshot<Vec<FirmwareUpgrade>>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum PendingUpgradesMessage {
    Firmware(SnapshotDelivery<Vec<FirmwareUpgrade>>),
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
        let PendingUpgradesMessage::Firmware(upgrades) = upgrades_message;
        self.firmware.receive(upgrades, clock);
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: PendingUpgradesInteraction,
    ) -> Task<PendingUpgradesMessage> {
        match interaction {
            PendingUpgradesInteraction::OpenTerminal(terminal) => terminal.open(),
            PendingUpgradesInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a, PendingUpgradesInteraction> {
        let summarised_row =
            GroupItem::reading(self.reading_row(clock)).with_hover_card(Box::new(move |_| {
                self.firmware.hover_card_with(self.hover_card())
            }));
        let group_summary = GroupSummary::showing(
            Phosphor::ArrowCircleUp(Weight::Regular).into(),
            &summarised_row,
        );
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PendingUpgrades,
            group_summary,
            &self.expansion,
            PendingUpgradesInteraction::Expansion,
            [
                Some(summarised_row),
                self.firmware_upgrade_item(config),
                self.firmware.failure().map(|failure| {
                    GroupItem::reading(failure.reading_row(IconGlyph::default())).with_hover_card(
                        Box::new(move |_| {
                            self.firmware.sections_described_by(|snapshot| {
                                firmware_upgrades_hover_card(snapshot)
                            })
                        }),
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
            .visible_reading(|upgrades| firmware_upgrades_panel_visibility(upgrades))
            .map(|upgrades| firmware_upgrades_badge(upgrades))]
    }

    fn category_states(&self, clock: Clock) -> [Option<ReadingDisplayState>; 1] {
        [self.firmware.shown_state(
            |snapshot| firmware_upgrades_state(snapshot),
            |snapshot| firmware_upgrades_panel_visibility(snapshot),
            clock,
        )]
    }
}

impl PendingUpgradesGroup {
    fn state(&self, clock: Clock) -> ReadingDisplayState {
        self.category_states(clock)
            .into_iter()
            .flatten()
            .map(ReadingDisplayState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Count",
            dictionary_table(
                self.firmware
                    .visible_reading(|upgrades| firmware_upgrades_panel_visibility(upgrades))
                    .map(|upgrades| {
                        LabeledValue::new(
                            firmware_upgrade_list::LABEL,
                            Quantity::count(upgrades.len()),
                        )
                    }),
            ),
        )]
    }

    fn reading_row(&self, clock: Clock) -> ReadingRow<'_> {
        let base = ReadingRow::glyph_only(
            "Available upgrades",
            Phosphor::ArrowCircleUp(Weight::Regular).into(),
            self.state(clock),
            Box::new(move |_| self.hover_card()),
        );
        let total: usize = self.category_badges().into_iter().flatten().sum();
        let badge = if total > 0 {
            ReadoutBadge::Counted(total)
        } else {
            ReadoutBadge::Unbadged
        };
        ReadingRow {
            readout: RowReadout {
                badge,
                ..base.readout
            },
            ..base
        }
    }
}
