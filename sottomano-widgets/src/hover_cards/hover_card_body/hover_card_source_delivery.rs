use std::time::{Duration, SystemTime};

use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::DeliveryCadence,
};

use super::{Heading, HoverCardBody, Section};
use crate::{clock::Clock, hover_cards::HoverAnnouncement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionsDeliveryState {
    Delivered { delivery_timing: DeliveryTiming },
    AwaitingDelivery,
}

#[derive(Debug, Clone, PartialEq)]
enum GroupHeading {
    Unheaded,
    Source(Heading),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourcedSections {
    heading: GroupHeading,
    delivery_state: SectionsDeliveryState,
    sections: Vec<Section>,
}

impl SourcedSections {
    #[must_use]
    pub fn delivered(body: HoverCardBody, delivery_timing: DeliveryTiming) -> Self {
        Self {
            heading: GroupHeading::Unheaded,
            delivery_state: SectionsDeliveryState::Delivered { delivery_timing },
            sections: body.sections,
        }
    }

    #[must_use]
    pub fn source_delivered<S: SnapshotSource>(
        body: HoverCardBody,
        delivery_timing: DeliveryTiming,
    ) -> Self {
        Self {
            heading: GroupHeading::Source(Heading::of_source::<S>()),
            delivery_state: SectionsDeliveryState::Delivered { delivery_timing },
            sections: body.sections,
        }
    }

    #[must_use]
    pub fn source_awaiting_delivery<S: SnapshotSource>(body: HoverCardBody) -> Self {
        Self {
            heading: GroupHeading::Source(Heading::of_source::<S>()),
            delivery_state: SectionsDeliveryState::AwaitingDelivery,
            sections: body.sections,
        }
    }

    #[must_use]
    pub fn source_heading(&self) -> Option<&Heading> {
        match &self.heading {
            GroupHeading::Source(heading) => Some(heading),
            GroupHeading::Unheaded => None,
        }
    }

    #[must_use]
    pub fn delivery_state(&self) -> SectionsDeliveryState {
        self.delivery_state
    }

    pub fn filled_sections(&self) -> impl Iterator<Item = &Section> {
        self.sections
            .iter()
            .filter(|section| !section.body.is_empty())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filled_sections().next().is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFreshness {
    Fresh,
    PastPollPeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTiming {
    PeriodicPolling {
        poll_period: Duration,
        delivered_at: SystemTime,
    },
    LivePushWithRecheck {
        recheck_period: Duration,
        delivered_at: SystemTime,
    },
    LivePush,
    SingleRead,
}

impl DeliveryTiming {
    #[must_use]
    pub fn from_snapshot_delivery<S: SnapshotSource>(
        delivery: &SnapshotDelivery<S>,
        clock: Clock,
    ) -> Option<Self> {
        match delivery {
            SnapshotDelivery::SnapshotDelivered {
                delivery_cadence, ..
            } => Some(Self::from_delivery_cadence(*delivery_cadence, clock)),
            SnapshotDelivery::SourceUnavailable
            | SnapshotDelivery::InternetUnreachable(_)
            | SnapshotDelivery::SnapshotUnchanged => None,
        }
    }

    #[must_use]
    pub fn from_delivery_cadence(delivery_cadence: DeliveryCadence, clock: Clock) -> Self {
        match delivery_cadence {
            DeliveryCadence::PeriodicPolling(polling) => DeliveryTiming::PeriodicPolling {
                poll_period: polling.poll_period(),
                delivered_at: clock.wall,
            },
            DeliveryCadence::LivePushWithRecheck(recheck) => DeliveryTiming::LivePushWithRecheck {
                recheck_period: recheck.poll_period(),
                delivered_at: clock.wall,
            },
            DeliveryCadence::LivePush => DeliveryTiming::LivePush,
            DeliveryCadence::SingleRead => DeliveryTiming::SingleRead,
        }
    }

    #[must_use]
    pub fn redelivered(self, clock: Clock) -> Self {
        match self {
            DeliveryTiming::PeriodicPolling { poll_period, .. } => {
                DeliveryTiming::PeriodicPolling {
                    poll_period,
                    delivered_at: clock.wall,
                }
            }
            DeliveryTiming::LivePushWithRecheck { recheck_period, .. } => {
                DeliveryTiming::LivePushWithRecheck {
                    recheck_period,
                    delivered_at: clock.wall,
                }
            }
            DeliveryTiming::LivePush | DeliveryTiming::SingleRead => self,
        }
    }

    #[must_use]
    pub fn freshness_at(self, clock: Clock) -> SnapshotFreshness {
        let (DeliveryTiming::PeriodicPolling {
            poll_period: refresh_period,
            delivered_at,
        }
        | DeliveryTiming::LivePushWithRecheck {
            recheck_period: refresh_period,
            delivered_at,
        }) = self
        else {
            return SnapshotFreshness::Fresh;
        };
        let elapsed = clock
            .wall
            .duration_since(delivered_at)
            .unwrap_or(Duration::ZERO);
        if elapsed >= refresh_period {
            SnapshotFreshness::PastPollPeriod
        } else {
            SnapshotFreshness::Fresh
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoverCard {
    pub groups: Vec<SourcedSections>,
    pub announcement: HoverAnnouncement,
}

impl HoverCard {
    #[must_use]
    pub fn empty() -> Self {
        Self::grouped([])
    }

    pub fn grouped(groups: impl IntoIterator<Item = SourcedSections>) -> Self {
        Self {
            groups: groups.into_iter().collect(),
            announcement: HoverAnnouncement::default(),
        }
    }

    #[must_use]
    pub fn sourced(body: HoverCardBody, delivery_timing: DeliveryTiming) -> Self {
        Self::grouped([SourcedSections::delivered(body, delivery_timing)])
    }

    #[must_use]
    pub fn with_groups(mut self, extra: impl IntoIterator<Item = SourcedSections>) -> Self {
        self.groups.extend(extra);
        self
    }

    pub fn filled_groups(&self) -> impl Iterator<Item = &SourcedSections> {
        self.groups.iter().filter(|group| !group.is_empty())
    }

    #[must_use]
    pub fn with_announcement(mut self, announcement: HoverAnnouncement) -> Self {
        self.announcement = announcement;
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filled_groups().next().is_none()
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::cpu_memory_compute::cpu_busy_processes::CpuBusyProcesses;

    use super::*;

    #[test]
    fn a_seeding_section_keeps_its_group_visible() {
        let hover_card = HoverCard::grouped([SourcedSections::source_awaiting_delivery::<
            CpuBusyProcesses,
        >(HoverCardBody::sections([
            Section::section_seeding(),
        ]))]);
        assert_eq!(hover_card.filled_groups().count(), 1);
    }

    #[test]
    fn a_source_group_carries_its_source_title_and_caption() {
        let headed = SourcedSections::source_delivered::<CpuBusyProcesses>(
            HoverCardBody::sections([]),
            DeliveryTiming::LivePush,
        );
        let heading = headed.source_heading().unwrap();
        assert_eq!(heading.title(), CpuBusyProcesses::NAME);
        assert!(heading.caption().is_some());
        let unheaded =
            SourcedSections::delivered(HoverCardBody::sections([]), DeliveryTiming::LivePush);
        assert!(unheaded.source_heading().is_none());
    }
}
