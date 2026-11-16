use std::collections::BTreeMap;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cgroup_memory_limits::{LimitedCgroup, LimitedCgroups},
    system_protocol_clients::sysfs_cgroup_memory::MemoryBound,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        group_summary::GroupSummary,
        terminal_command::TerminalCommand,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const STALL_WARNING_PERCENT: f64 = 10.0;
const CGTOP_PRESS_HINT: &str = "watch it in systemd-cgtop";

fn bound_text(memory_bound: MemoryBound) -> String {
    match memory_bound {
        MemoryBound::Unbounded => "none".to_owned(),
        MemoryBound::Bytes(bound_bytes) => bound_bytes.to_string(),
    }
}

struct LimitedCgroupRow<'a> {
    cgroup: &'a LimitedCgroup,
    killed_this_session: bool,
}

impl LimitedCgroupRow<'_> {
    fn detail_entries(&self) -> Vec<LabeledValue> {
        let memory = &self.cgroup.memory;
        let mut detail_entries = vec![
            LabeledValue::new("In use", memory.current.to_string()),
            LabeledValue::new("Throttled above", bound_text(memory.high)),
            LabeledValue::new("Hard cap", bound_text(memory.max)),
        ];
        if let Some(swap_current) = memory.swap_current {
            detail_entries.push(LabeledValue::new(
                "Swap",
                match memory.swap_max {
                    MemoryBound::Unbounded => swap_current.to_string(),
                    MemoryBound::Bytes(swap_cap) => format!("{swap_current} of {swap_cap}"),
                },
            ));
        }
        if let Some(stalled_percent) = memory.stalled_percent_last_ten_seconds {
            detail_entries.push(LabeledValue::new(
                "Stalled on memory (10 s)",
                format!("{stalled_percent:.1} %"),
            ));
        }
        detail_entries.extend([
            LabeledValue::new(
                "Times throttled",
                memory.event_counts.throttled_at_high.to_string(),
            ),
            LabeledValue::new(
                "Times at hard cap",
                memory.event_counts.reached_max.to_string(),
            ),
            LabeledValue::new(
                "Processes killed",
                memory.event_counts.oom_kills.to_string(),
            ),
            LabeledValue::new("Cgroup", self.cgroup.cgroup_path.clone()),
        ]);
        detail_entries
    }
}

impl<'a> LimitedCgroupRow<'a> {
    fn state(&self) -> IndicatorState {
        let memory = &self.cgroup.memory;
        if self.killed_this_session {
            IndicatorState::Critical
        } else if memory.exceeds_high()
            || memory
                .stalled_percent_last_ten_seconds
                .is_some_and(|stalled_percent| stalled_percent > STALL_WARNING_PERCENT)
        {
            IndicatorState::Warning
        } else {
            IndicatorState::On
        }
    }

    fn usage_text(&self) -> String {
        let memory = &self.cgroup.memory;
        match memory.limit() {
            Some(limit) => format!("{} / {limit}", memory.current),
            None => format!("{}, no cap", memory.current),
        }
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Memory",
            dictionary_table(self.detail_entries()),
        )]
    }

    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            self.cgroup.unit_name.clone(),
            Phosphor::Gauge(Weight::Regular).into(),
            self.state(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.usage_text()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }
}

#[derive(Default)]
pub struct CgroupMemoryLimitsGroup {
    listing: SubscribedSnapshot<LimitedCgroups>,
    first_seen_oom_kills: BTreeMap<String, u64>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum CgroupMemoryLimitsMessage {
    Cgroups(SnapshotDelivery<LimitedCgroups>),
}

#[derive(Debug, Clone)]
pub enum CgroupMemoryLimitsInteraction {
    OpenCgroupTop(TerminalCommand),
    Expansion(WidgetGroupExpansionMessage),
}

impl CgroupMemoryLimitsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self) -> Subscription<CgroupMemoryLimitsMessage> {
        snapshot_subscription::<LimitedCgroups>(()).map(CgroupMemoryLimitsMessage::Cgroups)
    }

    pub fn update(
        &mut self,
        message: CgroupMemoryLimitsMessage,
        clock: Clock,
    ) -> Task<CgroupMemoryLimitsMessage> {
        match message {
            CgroupMemoryLimitsMessage::Cgroups(delivery) => {
                self.listing.receive(delivery, clock);
                self.remember_first_oom_kill_counts();
                Task::none()
            }
        }
    }

    pub fn interact(
        &mut self,
        interaction: CgroupMemoryLimitsInteraction,
    ) -> Task<CgroupMemoryLimitsMessage> {
        match interaction {
            CgroupMemoryLimitsInteraction::OpenCgroupTop(cgroup_top) => cgroup_top.open(),
            CgroupMemoryLimitsInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn remember_first_oom_kill_counts(&mut self) {
        for cgroup in self
            .listing
            .get()
            .into_iter()
            .flat_map(|listing| listing.cgroups.iter())
        {
            self.first_seen_oom_kills
                .entry(cgroup.cgroup_path.clone())
                .or_insert(cgroup.memory.event_counts.oom_kills);
        }
    }

    fn listed_cgroups(&self) -> &[LimitedCgroup] {
        self.listing
            .get()
            .map(|listing| listing.cgroups.as_slice())
            .unwrap_or_default()
    }

    fn cgroup_row<'a>(&self, cgroup: &'a LimitedCgroup) -> LimitedCgroupRow<'a> {
        LimitedCgroupRow {
            cgroup,
            killed_this_session: self
                .first_seen_oom_kills
                .get(&cgroup.cgroup_path)
                .is_some_and(|first_seen| cgroup.memory.event_counts.oom_kills > *first_seen),
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> WidgetGroupContent<'a> {
        let rows = self.listed_cgroups().iter().map(|cgroup| {
            self.cgroup_row(cgroup)
                .indicator()
                .with_hover_card(Box::new(move |_| {
                    self.listing
                        .hover_card_with(self.cgroup_row(cgroup).hover_card())
                }))
                .on_press(
                    CGTOP_PRESS_HINT,
                    CgroupMemoryLimitsInteraction::OpenCgroupTop(TerminalCommand::running(
                        config,
                        [
                            "systemd-cgtop".to_owned(),
                            "--order=memory".to_owned(),
                            cgroup.cgroup_path.clone(),
                        ],
                    )),
                )
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::CgroupMemoryLimits,
            GroupSummary::glyph(
                Phosphor::ChartPieSlice(Weight::Regular).into(),
                self.state(),
            ),
            &self.expansion,
            CgroupMemoryLimitsInteraction::Expansion,
            rows,
        )
    }
}

impl CgroupMemoryLimitsGroup {
    fn state(&self) -> IndicatorState {
        self.listed_cgroups()
            .iter()
            .map(|cgroup| self.cgroup_row(cgroup).state().severity())
            .max()
            .unwrap_or_default()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytesize::ByteSize;
    use sottomano_desktop_sources::{
        cgroup_memory_limits::LimitedCgroup,
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        system_protocol_clients::sysfs_cgroup_memory::{
            CgroupMemorySample, MemoryBound, MemoryEventCounts,
        },
    };

    use super::*;

    fn capped_cgroup(current: ByteSize, oom_kills: u64) -> LimitedCgroup {
        LimitedCgroup {
            unit_name: "ci-runners.slice".to_owned(),
            cgroup_path: "/ci-runners.slice".to_owned(),
            memory: CgroupMemorySample {
                current,
                high: MemoryBound::Bytes(ByteSize::gib(10)),
                max: MemoryBound::Bytes(ByteSize::gib(12)),
                swap_current: Some(ByteSize::gib(1)),
                swap_max: MemoryBound::Bytes(ByteSize::gib(4)),
                stalled_percent_last_ten_seconds: Some(0.0),
                event_counts: MemoryEventCounts {
                    oom_kills,
                    ..MemoryEventCounts::default()
                },
            },
        }
    }

    fn deliver_cgroup(group: &mut CgroupMemoryLimitsGroup, cgroup: LimitedCgroup) {
        let _ = group.update(
            CgroupMemoryLimitsMessage::Cgroups(SnapshotDelivery::fixture_delivered(
                Ok(LimitedCgroups {
                    cgroups: vec![cgroup],
                }),
                DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(2))),
            )),
            Clock::real(),
        );
    }

    #[test]
    fn a_row_labels_usage_against_its_hard_cap() {
        let cgroup = capped_cgroup(ByteSize::gib(6), 0);
        let row = LimitedCgroupRow {
            cgroup: &cgroup,
            killed_this_session: false,
        };
        assert_eq!(
            row.usage_text(),
            format!("{} / {}", ByteSize::gib(6), ByteSize::gib(12))
        );
        assert_eq!(row.state(), IndicatorState::On);
    }

    #[test]
    fn usage_above_the_throttle_mark_warns() {
        let cgroup = capped_cgroup(ByteSize::gib(11), 0);
        let row = LimitedCgroupRow {
            cgroup: &cgroup,
            killed_this_session: false,
        };
        assert_eq!(row.state(), IndicatorState::Warning);
    }

    #[test]
    fn a_kill_after_the_first_indicator_is_critical() {
        let mut group = CgroupMemoryLimitsGroup::default();
        deliver_cgroup(&mut group, capped_cgroup(ByteSize::gib(6), 2));
        assert_ne!(group.state(), IndicatorState::Critical);
        deliver_cgroup(&mut group, capped_cgroup(ByteSize::gib(6), 3));
        assert_eq!(group.state(), IndicatorState::Critical);
    }
}
