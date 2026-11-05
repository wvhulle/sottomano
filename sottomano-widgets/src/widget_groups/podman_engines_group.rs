use std::{borrow::Cow, collections::BTreeMap};

use chrono::Locale;
use iced::{Subscription, alignment::Horizontal};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    podman_container_engine::libpod_running_containers::{
        EngineContainers, EngineReadError, EngineReadout, RunningContainer,
    },
};
use sottomano_toml_config::widget_groups::podman_engines::PodmanEnginesConfig;

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
        container_usage_text::{approximate_memory_text, cpu_load_text},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodmanEngineTarget {
    engine_label: String,
}

impl PodmanEngineTarget {
    #[must_use]
    pub fn of_engine(engine_label: &str) -> Self {
        Self {
            engine_label: engine_label.to_owned(),
        }
    }
}

impl From<PodmanEngineTarget> for HoverTarget {
    fn from(target: PodmanEngineTarget) -> Self {
        HoverTarget::PodmanEngine(target)
    }
}

impl IntoTableRecord<3> for &RunningContainer {
    fn into_table_record(self) -> TableRecord<3> {
        TableRecord::new(
            self.shown_name.clone(),
            [
                self.image.clone().into(),
                cpu_load_text(self.cpu_load).unwrap_or_default().into(),
                approximate_memory_text(self.resident_memory).into(),
            ],
        )
    }
}

struct PodmanEngineRow<'a> {
    engine_label: &'a str,
    readout: &'a EngineReadout,
}

fn running_summary(engine: &EngineContainers) -> String {
    format!("{} running", engine.containers.len())
}

fn containers_section(engine: &EngineContainers) -> Section {
    Section::column_table(
        [
            ColumnHeading::titled("Image").align_x(Horizontal::Left),
            ColumnHeading::titled("CPU"),
            ColumnHeading::titled("Memory"),
        ],
        engine.containers.iter(),
    )
}

impl GroupItemReading for PodmanEngineRow<'_> {
    fn name(&self) -> &'static str {
        "Podman engine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.engine_label.to_owned().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Cube(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(match self.readout {
            EngineReadout::Containers(engine) => running_summary(engine),
            EngineReadout::Unreadable(EngineReadError::SocketDenied) => {
                "no access: socket denied".to_owned()
            }
            EngineReadout::Unreadable(_) => "unreadable".to_owned(),
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.readout {
            EngineReadout::Containers(_) => ReadingDisplayState::Idle,
            EngineReadout::Unreadable(_) => ReadingDisplayState::Failed,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        match self.readout {
            EngineReadout::Containers(engine) if engine.containers.is_empty() => {
                HoverCardBody::sections([Section::note("No running containers")])
            }
            EngineReadout::Containers(engine) => {
                HoverCardBody::sections([containers_section(engine)])
            }
            EngineReadout::Unreadable(failure) => {
                HoverCardBody::sections([Section::error(failure)])
            }
        }
    }
}

fn is_hidden_engine(readout: &EngineReadout) -> bool {
    matches!(
        readout,
        EngineReadout::Unreadable(EngineReadError::SocketMissing)
    )
}

#[derive(Default)]
pub struct PodmanEnginesGroup {
    engines: BTreeMap<String, LatestSnapshot<EngineReadout, HidesFetchFailures>>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PodmanEnginesMessage {
    Engine(String, SnapshotDelivery<EngineReadout>),
    Expansion(WidgetGroupExpansionMessage),
}

impl PodmanEnginesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(config: &PodmanEnginesConfig) -> Subscription<PodmanEnginesMessage> {
        Subscription::batch(config.engines.iter().map(|engine| {
            snapshot_subscription::<EngineReadout>(engine.resolved_socket_path())
                .with(engine.label.clone())
                .map(|(engine_label, delivery)| {
                    PodmanEnginesMessage::Engine(engine_label, delivery)
                })
        }))
    }

    pub fn update(&mut self, message: PodmanEnginesMessage, clock: Clock) {
        match message {
            PodmanEnginesMessage::Engine(engine_label, delivery) => {
                self.engines
                    .entry(engine_label)
                    .or_default()
                    .receive(delivery, clock);
            }
            PodmanEnginesMessage::Expansion(message) => {
                self.expansion.update(message);
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &PodmanEngineTarget, clock: Clock) -> Option<HoverCard> {
        let reading = self.engines.get(&target.engine_label)?;
        let row = PodmanEngineRow {
            engine_label: &target.engine_label,
            readout: reading.get()?,
        };
        reading.hover_card_with(row.hover_card(clock))
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a PodmanEnginesConfig,
    ) -> WidgetGroupContent<'a, PodmanEnginesMessage> {
        let rows = config.engines.iter().filter_map(|engine| {
            let readout = self.engines.get(&engine.label)?.get()?;
            (!is_hidden_engine(readout)).then(|| {
                GroupItem::reading(
                    PodmanEngineRow {
                        engine_label: &engine.label,
                        readout,
                    },
                    PodmanEngineTarget::of_engine(&engine.label),
                )
            })
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PodmanEngines,
            &self.expansion,
            PodmanEnginesMessage::Expansion,
            rows,
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Cube(Weight::Regular).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_socket_is_hidden_but_a_denied_one_is_not() {
        assert!(is_hidden_engine(&EngineReadout::Unreadable(
            EngineReadError::SocketMissing
        )));
        assert!(!is_hidden_engine(&EngineReadout::Unreadable(
            EngineReadError::SocketDenied
        )));
    }

    #[test]
    fn an_idle_engine_reports_zero_running_without_usage() {
        assert_eq!(
            running_summary(&EngineContainers {
                containers: Vec::new()
            }),
            "0 running"
        );
    }
}
