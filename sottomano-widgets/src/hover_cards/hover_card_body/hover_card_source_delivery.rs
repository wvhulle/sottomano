use std::time::{Duration, Instant};

use sottomano_desktop_sources::delivery_cadence_drivers::DeliveryCadence;

use super::Section;
use crate::{clock::Clock, hover_cards::HoverAnnouncement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryStamp {
    pub cadence: DeliveryCadence,
    pub delivered_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshDeadline {
    DueAt {
        refresh_period: Duration,
        deadline: Instant,
    },
    NeverRefreshes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFreshness {
    Fresh,
    PastPollPeriod,
}

impl DeliveryStamp {
    #[must_use]
    pub fn delivered(cadence: DeliveryCadence, clock: Clock) -> Self {
        Self {
            cadence,
            delivered_at: clock.instant,
        }
    }

    #[must_use]
    pub fn redelivered(self, clock: Clock) -> Self {
        Self {
            delivered_at: clock.instant,
            ..self
        }
    }

    #[must_use]
    pub fn refresh_deadline(self) -> RefreshDeadline {
        match self.cadence {
            DeliveryCadence::PeriodicPolling(polling)
            | DeliveryCadence::LivePushWithRecheck(polling) => RefreshDeadline::DueAt {
                refresh_period: polling.poll_period(),
                deadline: self.delivered_at + polling.poll_period(),
            },
            DeliveryCadence::LivePush | DeliveryCadence::SingleRead => {
                RefreshDeadline::NeverRefreshes
            }
        }
    }

    #[must_use]
    pub fn freshness_at(self, clock: Clock) -> SnapshotFreshness {
        match self.refresh_deadline() {
            RefreshDeadline::DueAt { deadline, .. } if clock.instant >= deadline => {
                SnapshotFreshness::PastPollPeriod
            }
            RefreshDeadline::DueAt { .. } | RefreshDeadline::NeverRefreshes => {
                SnapshotFreshness::Fresh
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDelivery {
    Delivered(DeliveryStamp),
    Loading,
    Failed,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionSource {
    Local,
    Snapshot {
        source_name: &'static str,
        delivery: SourceDelivery,
    },
}

#[derive(Debug, Clone)]
pub struct HoverCard {
    sections: Vec<Section>,
    announcement: HoverAnnouncement,
}

impl HoverCard {
    pub fn new(
        sections: impl IntoIterator<Item = Section>,
        announcement: HoverAnnouncement,
    ) -> Option<Self> {
        let shown_sections: Vec<Section> = sections
            .into_iter()
            .chain(Section::press_actions(announcement.press_hints))
            .filter(|section| !section.widget().is_empty())
            .collect();
        if shown_sections.is_empty() {
            return None;
        }
        Some(Self {
            sections: shown_sections,
            announcement,
        })
    }

    pub fn sections(&self) -> impl Iterator<Item = &Section> {
        self.sections.iter()
    }

    #[must_use]
    pub fn announcement(&self) -> &HoverAnnouncement {
        &self.announcement
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::{
        SnapshotSource, cpu_memory_compute::cpu_busy_processes::CpuBusyProcesses,
    };

    use super::*;
    use crate::hover_cards::{PressHints, hover_card_body::SectionWidget};

    #[test]
    fn a_card_needs_a_shown_section() {
        assert!(HoverCard::new([], HoverAnnouncement::default()).is_none());
    }

    #[test]
    fn press_hints_alone_make_a_card() {
        let announcement = HoverAnnouncement::from_reading(PressHints {
            left: Some("switch something"),
            ..PressHints::default()
        });
        let hover_card = HoverCard::new([], announcement).expect("an actions section");
        assert_eq!(
            hover_card
                .sections()
                .map(|section| section.title().text().to_owned())
                .collect::<Vec<_>>(),
            ["Actions"]
        );
    }

    #[test]
    fn a_sourced_section_names_its_source() {
        let section = Section::new("Count", SectionWidget::paragraph("counted"))
            .stamped_by::<CpuBusyProcesses>(SourceDelivery::Loading);
        assert_eq!(
            section.source(),
            SectionSource::Snapshot {
                source_name: CpuBusyProcesses::NAME,
                delivery: SourceDelivery::Loading,
            }
        );
    }
}
