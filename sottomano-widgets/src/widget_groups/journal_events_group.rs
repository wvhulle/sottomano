use iced::{Subscription, Task};

use crate::{widget_groups::widget_group_identity::WidgetGroupFamily, widgets::phosphor::Weight};

mod coredump_log;
mod kernel_error_log;
mod oom_kill_log;
mod service_error_log;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    systemd_units_journal::{
        journal_coredumps::CoredumpEntry, journal_kernel_errors::KernelOccurrence,
        journal_oom_kills::OomKillEntry, journal_service_errors::ServiceErrors,
    },
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
        hover_card_body::{HoverCardBody, Section, hover_card_source_delivery::HoverCard},
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

pub const MESSAGE_CAP: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalTarget {
    Events,
    Kernel,
    ServiceErrors,
    Coredumps,
    OomKills,
}

impl From<JournalTarget> for HoverTarget {
    fn from(target: JournalTarget) -> Self {
        HoverTarget::Journal(target)
    }
}

#[derive(Default)]
pub struct JournalEventsGroup {
    kernel: LatestSnapshot<Vec<KernelOccurrence>>,
    service_errors: LatestSnapshot<ServiceErrors>,
    coredumps: LatestSnapshot<Vec<CoredumpEntry>>,
    oom_kills: LatestSnapshot<Vec<OomKillEntry>>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum JournalEventsMessage {
    Kernel(SnapshotDelivery<Vec<KernelOccurrence>>),
    ServiceErrors(SnapshotDelivery<ServiceErrors>),
    Coredumps(SnapshotDelivery<Vec<CoredumpEntry>>),
    OomKills(SnapshotDelivery<Vec<OomKillEntry>>),
    Interaction(JournalEventsInteraction),
}

#[derive(Debug, Clone)]
pub enum JournalEventsInteraction {
    OpenLog(TerminalCommand),
    Expansion(WidgetGroupExpansionMessage),
}

impl JournalEventsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription() -> Subscription<JournalEventsMessage> {
        Subscription::batch([
            snapshot_subscription::<Vec<KernelOccurrence>>(()).map(JournalEventsMessage::Kernel),
            snapshot_subscription::<ServiceErrors>(()).map(JournalEventsMessage::ServiceErrors),
            snapshot_subscription::<Vec<CoredumpEntry>>(()).map(JournalEventsMessage::Coredumps),
            snapshot_subscription::<Vec<OomKillEntry>>(()).map(JournalEventsMessage::OomKills),
        ])
    }

    pub fn update(
        &mut self,
        journal_message: JournalEventsMessage,
        clock: Clock,
    ) -> Task<JournalEventsMessage> {
        match journal_message {
            JournalEventsMessage::Kernel(delivery) => {
                self.kernel.receive(delivery, clock);
            }
            JournalEventsMessage::ServiceErrors(delivery) => {
                self.service_errors.receive(delivery, clock);
            }
            JournalEventsMessage::Coredumps(delivery) => {
                self.coredumps.receive(delivery, clock);
            }
            JournalEventsMessage::OomKills(delivery) => {
                self.oom_kills.receive(delivery, clock);
            }
            JournalEventsMessage::Interaction(interaction) => return self.interact(interaction),
        }
        Task::none()
    }

    fn interact(&mut self, interaction: JournalEventsInteraction) -> Task<JournalEventsMessage> {
        match interaction {
            JournalEventsInteraction::OpenLog(journal) => journal.open(),
            JournalEventsInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: JournalTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            JournalTarget::Kernel => self.kernel.hover_card(clock),
            JournalTarget::ServiceErrors => self.service_errors.hover_card(clock),
            JournalTarget::Coredumps => self.coredumps.hover_card(clock),
            JournalTarget::OomKills => self.oom_kills.hover_card(clock),
            JournalTarget::Events => self
                .kernel
                .hover_card_with(self.hover_card(clock))
                .or_else(|| self.service_errors.hover_card_with(self.hover_card(clock)))
                .or_else(|| self.coredumps.hover_card_with(self.hover_card(clock)))
                .or_else(|| self.oom_kills.hover_card_with(self.hover_card(clock))),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, JournalEventsInteraction> {
        let has_kinds = self.kinds().iter().any(Option::is_some);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::JournalEvents,
            &self.expansion,
            JournalEventsInteraction::Expansion,
            [
                has_kinds.then(|| {
                    GroupItem::reading(self as &dyn GroupItemReading, JournalTarget::Events)
                }),
                self.kernel_log_item(config),
                self.service_errors
                    .visible_reading()
                    .map(|service_errors| {
                        GroupItem::reading(service_errors, JournalTarget::ServiceErrors).on_press(
                            TerminalCommand::JOURNAL_PRESS_HINT,
                            JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                                config,
                                service_errors.follow_log_args(),
                            )),
                        )
                    })
                    .or_else(|| {
                        self.service_errors.failure().map(|failure| {
                            GroupItem::reading(failure, JournalTarget::ServiceErrors)
                        })
                    }),
                self.coredump_log_item(config),
                self.oom_kill_log_item(config),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn kinds(&self) -> [Option<&dyn GroupItemReading>; 4] {
        [
            self.kernel.display(),
            self.service_errors.display(),
            self.coredumps.display(),
            self.oom_kills.display(),
        ]
    }
}

impl GroupItemReading for JournalEventsGroup {
    fn name(&self) -> &'static str {
        "Journal events"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Scroll(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        let total: u32 = self
            .kinds()
            .into_iter()
            .flatten()
            .filter_map(GroupItemReading::badge)
            .sum();
        (total > 0).then_some(total)
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        self.kinds()
            .into_iter()
            .flatten()
            .map(|kind| kind.state(clock).severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let kernel = self
            .kernel
            .visible_reading()
            .map(|errors| LabeledValue::new("Kernel errors", Quantity::count(errors.len())));
        let service = self
            .service_errors
            .visible_reading()
            .map(|errors| LabeledValue::new("Service errors", Quantity::count(errors.entry_count)));
        let coredumps = self
            .coredumps
            .visible_reading()
            .map(|coredumps| LabeledValue::new("Coredumps", Quantity::count(coredumps.len())));
        let oom_kills = self
            .oom_kills
            .visible_reading()
            .map(|kills| LabeledValue::new("OOM kills", Quantity::count(kills.len())));
        HoverCardBody::sections([Section::dictionary(
            kernel
                .into_iter()
                .chain(service)
                .chain(coredumps)
                .chain(oom_kills),
        )
        .titled("Last hour")])
    }
}
