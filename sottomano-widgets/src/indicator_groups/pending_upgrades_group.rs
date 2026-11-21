use std::iter;

use iced::{Subscription, Task};

use crate::{
    glyph_icons::phosphor::Weight,
    group_indicators::{group_indicator::GroupIndicator, indicator_drawing::ReadoutBadge},
    hover_cards::hover_card_body::dictionary_table,
    indicator_groups::indicator_group_identity::IndicatorGroupFamily,
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
    glyph_icons::phosphor::Phosphor,
    group_indicators::indicator_state::IndicatorState,
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        terminal_command::TerminalCommand,
    },
    widgets::{
        localized_quantities::Quantity, subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const FIRMWARE_UPGRADES_GLYPH: Phosphor = Phosphor::Circuitry(Weight::Regular);

#[derive(Default)]
pub struct PendingUpgradesGroup {
    firmware: SubscribedSnapshot<Vec<FirmwareUpgrade>>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum PendingUpgradesMessage {
    Firmware(SnapshotDelivery<Vec<FirmwareUpgrade>>),
}

#[derive(Debug, Clone)]
pub enum PendingUpgradesInteraction {
    OpenTerminal(TerminalCommand),
    Expansion(IndicatorGroupExpansionMessage),
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

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        let mut rows = Vec::new();
        if let Some(upgrades) = self
            .firmware
            .visible_indicator(|upgrades| firmware_upgrades_panel_visibility(upgrades))
        {
            rows.push(self.firmware_upgrade_indicator(upgrades, config));
        }
        if let Some(placeholder) = self.firmware.failure() {
            rows.push(
                placeholder
                    .indicator(FIRMWARE_UPGRADES_GLYPH.into())
                    .with_hover_card(Box::new(move |_| {
                        self.firmware.sections_described_by(|snapshot| {
                            firmware_upgrades_hover_card(snapshot)
                        })
                    })),
            );
        }
        let build_pending_upgrades_indicator = || {
            self.indicator().with_hover_card(Box::new(move |_| {
                self.firmware.hover_card_with(self.hover_card())
            }))
        };
        let headline_indicator = build_pending_upgrades_indicator()
            .with_headline_glyph(Phosphor::ArrowFatLinesUp(Weight::Regular).into());
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::PendingUpgrades,
            headline_indicator,
            &self.expansion,
            PendingUpgradesInteraction::Expansion,
            iter::once(build_pending_upgrades_indicator()).chain(rows),
        )
    }

    fn category_badges(&self) -> [Option<usize>; 1] {
        [self
            .firmware
            .visible_indicator(|upgrades| firmware_upgrades_panel_visibility(upgrades))
            .map(|upgrades| firmware_upgrades_badge(upgrades))]
    }

    fn category_states(&self) -> [Option<IndicatorState>; 1] {
        [self.firmware.shown_state(
            |snapshot| firmware_upgrades_state(snapshot),
            |snapshot| firmware_upgrades_panel_visibility(snapshot),
        )]
    }
}

impl PendingUpgradesGroup {
    fn state(&self) -> IndicatorState {
        self.category_states()
            .into_iter()
            .flatten()
            .map(IndicatorState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Count",
            dictionary_table(
                self.firmware
                    .visible_indicator(|upgrades| firmware_upgrades_panel_visibility(upgrades))
                    .map(|upgrades| {
                        LabeledValue::new(
                            firmware_upgrade_list::LABEL,
                            Quantity::count(upgrades.len()),
                        )
                    }),
            ),
        )]
    }

    fn indicator(&self) -> GroupIndicator<'_> {
        let base = GroupIndicator::glyph_only(
            "Available upgrades",
            Phosphor::ArrowCircleUp(Weight::Regular).into(),
            self.state(),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()));
        let total: usize = self.category_badges().into_iter().flatten().sum();
        let badge = if total > 0 {
            ReadoutBadge::Counted(total)
        } else {
            ReadoutBadge::Unbadged
        };
        base.with_badge(badge)
    }
}
