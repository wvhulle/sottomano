use std::{borrow::Cow, path::Path};

use iced::Task;
use sottomano_desktop_sinks::{
    container_power::{ContainerEngineApi, start_container, stop_container},
    sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    container_run_state::{ContainerRunState, RunChange},
    podman_container_engine::{
        libpod_running_containers::{
            EngineListing, EngineReadError, RunningContainer, StoppedContainer,
        },
        systemd_service_engine_discovery::EngineOwner,
    },
};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
        toggle_control::SwitchPosition,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, dictionary_table},
    },
    widget_groups::{
        process_usage_cells::{approximate_memory_text, cpu_share_quantity, uptime_entry},
        widget_group_content::WidgetGroupExpansionMessage,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const SHOWN_CONTAINER_ROWS: usize = 12;
const START_HINT: &str = "start the container";
const STOP_HINT: &str = "stop the container";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineContainerTarget {
    pub socket_path: Cow<'static, str>,
    pub container_id: Cow<'static, str>,
}

impl EngineContainerTarget {
    pub(crate) fn names_socket(&self, socket_path: &Path) -> bool {
        Path::new(self.socket_path.as_ref()) == socket_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSwitchRequest {
    pub target: EngineContainerTarget,
    pub change: RunChange,
}

#[derive(Debug, Clone)]
pub enum EngineContainerInteraction {
    SwitchContainer(ContainerSwitchRequest),
    Expansion(WidgetGroupExpansionMessage),
}

pub(crate) fn container_switch_task<Message: Send + 'static>(
    engine_api: ContainerEngineApi,
    request: ContainerSwitchRequest,
    refused: fn(GuestPowerError) -> Message,
) -> Task<Message> {
    Task::future(async move {
        let EngineContainerTarget {
            socket_path,
            container_id,
        } = request.target;
        let socket_path = Path::new(socket_path.as_ref());
        let switched = match request.change {
            RunChange::Start => start_container(engine_api, socket_path, &container_id).await,
            RunChange::Stop => stop_container(engine_api, socket_path, &container_id).await,
        };
        switched.err()
    })
    .and_then(move |refusal| Task::done(refused(refusal)))
}

pub(crate) fn engine_failure_label(read_failure: &EngineReadError) -> &'static str {
    match read_failure {
        EngineReadError::SocketDenied => "no access: socket denied",
        _ => "unreadable",
    }
}

pub(crate) fn engine_container_name(
    container_name: &str,
    owner: &EngineOwner,
) -> Cow<'static, str> {
    match owner {
        EngineOwner::YourUser => container_name.to_owned().into(),
        EngineOwner::Root => format!("{container_name} as root").into(),
        EngineOwner::ServiceUser(user_name) => format!("{container_name} as {user_name}").into(),
    }
}

pub(crate) struct RunningContainerRow<'a> {
    pub(crate) container: &'a RunningContainer,
    pub(crate) owner: &'a EngineOwner,
}

impl<'a> RunningContainerRow<'a> {
    pub(crate) fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(approximate_memory_text(
                    self.container.resident_memory,
                )),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                engine_container_name(&self.container.container_name, self.owner),
                Phosphor::Package(Weight::Regular).into(),
                ContainerRunState::Running.into(),
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    pub(crate) fn hover_card(&self, clock: Clock) -> Vec<Section> {
        let container = self.container;
        let mut detail_entries = vec![
            LabeledValue::new("State", ContainerRunState::Running.label()),
            LabeledValue::new("Image", container.image.clone()),
        ];
        if let Some(cpu_share) = cpu_share_quantity(container.cpu_load) {
            detail_entries.push(LabeledValue::new("CPU load", cpu_share));
        }
        detail_entries.push(LabeledValue::new(
            "Memory use",
            approximate_memory_text(container.resident_memory),
        ));
        detail_entries.extend(uptime_entry(clock, container.started_at));
        detail_entries.extend([
            LabeledValue::new("Name", container.container_name.clone()),
            LabeledValue::new("Engine", self.owner.to_string()),
        ]);
        vec![Section::new("Container", dictionary_table(detail_entries))]
    }
}

pub(crate) struct StoppedContainerRow<'a> {
    pub(crate) container: &'a StoppedContainer,
    pub(crate) owner: &'a EngineOwner,
}

impl<'a> StoppedContainerRow<'a> {
    pub(crate) fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(ContainerRunState::Stopped.label().to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                engine_container_name(&self.container.container_name, self.owner),
                Phosphor::Package(Weight::Regular).into(),
                ContainerRunState::Stopped.into(),
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(crate) fn hover_card(&self) -> Vec<Section> {
        let container = self.container;
        vec![Section::new(
            "Container",
            dictionary_table([
                LabeledValue::new("State", ContainerRunState::Stopped.label()),
                LabeledValue::new("Image", container.image.clone()),
                LabeledValue::new("Name", container.container_name.clone()),
                LabeledValue::new("Engine", self.owner.to_string()),
            ]),
        )]
    }
}

pub(crate) struct HiddenContainersRow {
    pub(crate) hidden_count: usize,
}

impl HiddenContainersRow {
    pub(crate) fn reading_row(self) -> ReadingRow<'static> {
        ReadingRow::glyph_only(
            format!("{} more", self.hidden_count),
            Phosphor::DotsThree(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(move |_| self.hover_card()),
        )
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Hidden",
            dictionary_table([LabeledValue::new(
                "Not shown",
                self.hidden_count.to_string(),
            )]),
        )]
    }
}

pub(crate) struct EngineContainers<'a> {
    pub(crate) socket_path: &'a Path,
    pub(crate) owner: &'a EngineOwner,
    pub(crate) listing: &'a EngineListing,
}

impl<'a> EngineContainers<'a> {
    fn target_of(&self, container_id: &str) -> EngineContainerTarget {
        EngineContainerTarget {
            socket_path: Cow::Owned(self.socket_path.display().to_string()),
            container_id: Cow::Owned(container_id.to_owned()),
        }
    }

    pub(crate) fn running_rows<Message: 'a>(
        &self,
        target_of: fn(EngineContainerTarget) -> HoverTarget,
        switched: fn(ContainerSwitchRequest) -> Message,
    ) -> Vec<GroupItem<'a, Message>> {
        let owner = self.owner;
        self.listing
            .running
            .containers
            .iter()
            .map(|container| {
                let target = self.target_of(&container.id);
                with_container_switch(
                    GroupItem::reading(
                        RunningContainerRow { container, owner }.reading_row(),
                        target_of(target.clone()),
                    ),
                    ContainerRunState::Running,
                    target,
                    switched,
                )
            })
            .collect()
    }

    pub(crate) fn stopped_rows<Message: 'a>(
        &self,
        target_of: fn(EngineContainerTarget) -> HoverTarget,
        switched: fn(ContainerSwitchRequest) -> Message,
    ) -> Vec<GroupItem<'a, Message>> {
        let owner = self.owner;
        self.listing
            .stopped
            .iter()
            .map(|container| {
                let target = self.target_of(&container.id);
                with_container_switch(
                    GroupItem::reading(
                        StoppedContainerRow { container, owner }.reading_row(),
                        target_of(target.clone()),
                    ),
                    ContainerRunState::Stopped,
                    target,
                    switched,
                )
            })
            .collect()
    }

    pub(crate) fn hover_body_of(&self, container_id: &str, clock: Clock) -> Option<Vec<Section>> {
        let owner = self.owner;
        if let Some(container) = self
            .listing
            .running
            .containers
            .iter()
            .find(|container| container.id == container_id)
        {
            return Some(RunningContainerRow { container, owner }.hover_card(clock));
        }
        let container = self
            .listing
            .stopped
            .iter()
            .find(|container| container.id == container_id)?;
        Some(StoppedContainerRow { container, owner }.hover_card())
    }
}

pub(crate) fn with_container_switch<'a, Message: 'a>(
    row: GroupItem<'a, Message>,
    run_state: ContainerRunState,
    target: EngineContainerTarget,
    switched: fn(ContainerSwitchRequest) -> Message,
) -> GroupItem<'a, Message> {
    match run_state {
        ContainerRunState::Running => row.on_switch_press(
            SwitchPosition::On,
            STOP_HINT,
            switched(ContainerSwitchRequest {
                target,
                change: RunChange::Stop,
            }),
        ),
        ContainerRunState::Stopped => row.on_switch_press(
            SwitchPosition::Off,
            START_HINT,
            switched(ContainerSwitchRequest {
                target,
                change: RunChange::Start,
            }),
        ),
    }
}

pub(crate) fn rows_ending_in_hidden_count<'a, Message: 'a>(
    mut rows: Vec<GroupItem<'a, Message>>,
    hidden_target: HoverTarget,
) -> Vec<GroupItem<'a, Message>> {
    if rows.len() <= SHOWN_CONTAINER_ROWS {
        return rows;
    }
    let hidden_count = rows.split_off(SHOWN_CONTAINER_ROWS - 1).len();
    rows.push(GroupItem::reading(
        HiddenContainersRow { hidden_count }.reading_row(),
        hidden_target,
    ));
    rows
}

#[cfg(test)]
mod tests {

    use super::HiddenContainersRow;

    #[test]
    fn the_hidden_row_counts_what_it_replaces() {
        let hidden = HiddenContainersRow { hidden_count: 4 };
        let hidden_row = hidden.reading_row();
        assert_eq!(hidden_row.face.name, "4 more");
    }
}
