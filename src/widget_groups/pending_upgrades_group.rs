use iced::{Subscription, Task};

pub mod firmware_upgrade_list;

use crate::{
    config::Config,
    data::source::fwupd::{self, PendingUpgrades},
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::{
        pending_upgrades_group::firmware_upgrade_list::FirmwareUpgradeList,
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::{Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, WidgetGroup},
        widget_state::WidgetState,
    },
};

#[derive(Default)]
pub struct PendingUpgradesGroup {
    firmware: LastPoll<FirmwareUpgradeList>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PendingUpgradesMessage {
    Firmware(PendingUpgrades),
    OpenTerminal(TerminalCommand),
    Expansion(ExpansionMessage),
}

impl PendingUpgradesGroup {
    pub fn subscription() -> Subscription<PendingUpgradesMessage> {
        fwupd::subscription().map(PendingUpgradesMessage::Firmware)
    }

    pub fn update(&mut self, msg: PendingUpgradesMessage) -> Task<PendingUpgradesMessage> {
        match msg {
            PendingUpgradesMessage::Firmware(Ok(upgrades)) => {
                self.firmware
                    .load(FirmwareUpgradeList::from_upgrades(upgrades));
            }
            PendingUpgradesMessage::Firmware(Err(failure)) => {
                self.firmware.receive(Err(failure));
            }
            PendingUpgradesMessage::OpenTerminal(terminal) => return terminal.open(),
            PendingUpgradesMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::FirmwareUpgrades => self.firmware.display()?.tooltip(),
            HoverTarget::PendingUpgrades => self.tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
    ) -> Option<WidgetGroup<'a, PendingUpgradesMessage>> {
        if !self.categories().iter().any(Option::is_some) {
            return None;
        }
        WidgetGroup::new(
            &self.expansion,
            PendingUpgradesMessage::Expansion,
            config,
            GroupHandle {
                face: HandleFace::Item(GroupItem::reading(self, HoverTarget::PendingUpgrades)),
                preview: None,
                forwards_scroll: false,
            },
            &[
                self.firmware
                    .get()
                    .map(|firmware| firmware.group_item(config)),
                self.firmware
                    .failure()
                    .map(|failure| GroupItem::reading(failure, HoverTarget::FirmwareUpgrades)),
            ],
        )
    }

    fn categories(&self) -> [Option<&dyn BarWidget>; 1] {
        [self.firmware.display()]
    }
}

impl BarWidget for PendingUpgradesGroup {
    fn icon(&self) -> Glyph {
        Phosphor::ArrowCircleUp(self.state().weight()).into()
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

    fn state(&self) -> WidgetState {
        self.categories()
            .into_iter()
            .flatten()
            .map(|category| category.state().severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled(&[(
            firmware_upgrade_list::LABEL,
            self.firmware
                .get()
                .map(|firmware| firmware.count().to_string()),
        )])])
    }
}
