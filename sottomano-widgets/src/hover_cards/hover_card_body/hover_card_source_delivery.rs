use std::time::{Duration, Instant};

use sottomano_desktop_sources::delivery_cadence_drivers::DeliveryCadence;

use super::Section;
use crate::{clock::Clock, hover_cards::CardHeader};

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
                refresh_period: polling.poll_period,
                deadline: self.delivered_at + polling.poll_period,
            },
            DeliveryCadence::LivePush | DeliveryCadence::SingleRead => {
                RefreshDeadline::NeverRefreshes
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDelivery {
    Delivered(DeliveryStamp),
    Loading,
    Failed,
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
    pub sections: Vec<Section>,
    pub header: CardHeader,
}

impl HoverCard {
    pub fn new(sections: impl IntoIterator<Item = Section>, header: CardHeader) -> Option<Self> {
        let shown_sections: Vec<Section> = sections
            .into_iter()
            .filter(|section| !section.widget.is_empty())
            .collect();
        if shown_sections.is_empty() {
            return None;
        }
        Some(Self {
            sections: shown_sections,
            header,
        })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::{
        SnapshotSource, cpu_memory_compute::cpu_busy_processes::CpuBusyProcesses,
    };

    use super::*;
    use crate::hover_cards::{MouseGesture, PressAction, hover_card_body::SectionWidget};

    #[test]
    fn a_card_needs_a_shown_section() {
        assert!(HoverCard::new([], CardHeader::Unheaded).is_none());
    }

    #[test]
    fn press_actions_alone_make_a_card() {
        let hover_card = HoverCard::new(
            Section::press_actions(vec![PressAction::new(
                MouseGesture::LeftClick,
                "switch something",
            )]),
            CardHeader::Unheaded,
        )
        .expect("an actions section");
        assert_eq!(
            hover_card
                .sections
                .iter()
                .map(|section| section.title.text().to_owned())
                .collect::<Vec<_>>(),
            ["Actions"]
        );
    }

    #[test]
    fn a_sourced_section_names_its_source() {
        let section = Section::new("Count", SectionWidget::paragraph("counted"))
            .stamped_by::<CpuBusyProcesses>(SourceDelivery::Loading);
        assert_eq!(
            section.source,
            SectionSource::Snapshot {
                source_name: CpuBusyProcesses::NAME,
                delivery: SourceDelivery::Loading,
            }
        );
    }
}
