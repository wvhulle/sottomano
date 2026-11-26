use std::{collections::HashMap, sync::Arc};

use super::{
    AudioState, ChannelLevels, DefaultChannel, DeviceDescription, Direction, MuteState, OutputKind,
    WindowAudio, level_command_writes::LevelChange,
};

#[derive(Debug, Default, PartialEq)]
pub(super) struct Levels {
    pub(super) channel_volumes: Vec<f32>,
    pub(super) mute_state: MuteState,
}

impl Levels {
    pub(super) fn percent(&self) -> i64 {
        let Some(&cubic) = self.channel_volumes.first() else {
            return 0;
        };
        #[allow(clippy::cast_possible_truncation)]
        let percent = (cubic.cbrt() * 100.0).round() as i64;
        percent.clamp(0, 100)
    }

    pub(super) fn merge(&mut self, update: LevelsUpdate) {
        for change in update.changes {
            match change {
                LevelChange::Volumes(volumes) => self.channel_volumes = volumes,
                LevelChange::Mute(mute_state) => self.mute_state = mute_state,
            }
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct LevelsUpdate {
    pub(super) changes: Vec<LevelChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KindHint {
    Hinted(OutputKind),
    Unhinted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NodeDevice {
    Device(u32),
    Deviceless,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum NodePlacement {
    Routed(RoutePlacement),
    Unrouted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StreamProcess {
    Process(i32),
    Unidentified,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RoutePlacement {
    pub(super) device: u32,
    pub(super) route_device: u32,
}

#[derive(Debug)]
pub(super) struct AudioNode {
    pub(super) direction: Direction,
    pub(super) name: String,
    pub(super) description: DeviceDescription,
    pub(super) device: NodeDevice,
    pub(super) props_kind: KindHint,
    pub(super) serial: u64,
    pub(super) placement: NodePlacement,
    pub(super) levels: Levels,
}

#[derive(Debug)]
pub(super) struct Route {
    pub(super) index: u32,
    pub(super) port_kind: KindHint,
    pub(super) levels: Levels,
}

#[derive(Debug)]
pub(super) struct AppStream {
    pub(super) process: StreamProcess,
    pub(super) stream_activity: StreamActivity,
    pub(super) levels: Levels,
}

#[derive(Debug, Default)]
pub(super) struct AudioGraph {
    pub(super) nodes: HashMap<u32, AudioNode>,
    pub(super) streams: HashMap<u32, AppStream>,
    routes: HashMap<u32, HashMap<u32, Route>>,
    device_kinds: HashMap<u32, OutputKind>,
    default_names: HashMap<Direction, String>,
}

pub(super) struct DefaultNode<'graph> {
    pub(super) id: u32,
    pub(super) node: &'graph AudioNode,
}

impl AudioGraph {
    pub(super) fn add_node(&mut self, id: u32, node: AudioNode) {
        self.nodes.insert(id, node);
    }

    pub(super) fn set_placement(&mut self, id: u32, placement: RoutePlacement) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.placement = NodePlacement::Routed(placement);
        }
    }

    pub(super) fn set_kind_hint(&mut self, id: u32, kind: OutputKind) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.props_kind = KindHint::Hinted(kind);
        }
    }

    pub(super) fn set_device_kind(&mut self, id: u32, kind: OutputKind) {
        self.device_kinds.insert(id, kind);
    }

    pub(super) fn merge_levels(&mut self, id: u32, update: LevelsUpdate) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.levels.merge(update);
        } else if let Some(stream) = self.streams.get_mut(&id) {
            stream.levels.merge(update);
        }
    }

    pub(super) fn add_stream(&mut self, id: u32, stream: AppStream) {
        self.streams.insert(id, stream);
    }

    pub(super) fn set_stream_info(
        &mut self,
        id: u32,
        process: StreamProcess,
        stream_activity: StreamActivity,
    ) {
        if let Some(stream) = self.streams.get_mut(&id) {
            if let StreamProcess::Process(_) = process {
                stream.process = process;
            }
            stream.stream_activity = stream_activity;
        }
    }

    pub(super) fn merge_route_levels(&mut self, device: u32, route: RouteLevels) {
        let entry = self
            .routes
            .entry(device)
            .or_default()
            .entry(route.route_device)
            .or_insert_with(|| Route {
                index: route.index,
                port_kind: KindHint::Unhinted,
                levels: Levels::default(),
            });
        entry.index = route.index;
        entry.port_kind = route.port_kind;
        entry.levels.merge(route.update);
    }

    pub(super) fn set_default(&mut self, change: DefaultNodeChange) {
        match change {
            DefaultNodeChange::Named { direction, name } => {
                self.default_names.insert(direction, name);
            }
            DefaultNodeChange::Cleared { direction } => {
                self.default_names.remove(&direction);
            }
        }
    }

    pub(super) fn remove_object(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.streams.remove(&id);
        self.routes.remove(&id);
        self.device_kinds.remove(&id);
    }

    pub(super) fn default_name(&self, direction: Direction) -> Option<&str> {
        self.default_names.get(&direction).map(String::as_str)
    }

    pub(super) fn node_count(&self, direction: Direction) -> usize {
        self.nodes
            .values()
            .filter(|node| node.direction == direction)
            .count()
    }

    pub(super) fn default_node(&self, direction: Direction) -> Option<DefaultNode<'_>> {
        let name = self.default_name(direction)?;
        self.nodes
            .iter()
            .filter(|(_, node)| node.name == name)
            .max_by_key(|(id, node)| (node.serial, **id))
            .map(|(id, node)| DefaultNode { id: *id, node })
    }

    pub(super) fn active_route(&self, node: &AudioNode) -> Option<&Route> {
        let NodePlacement::Routed(placement) = &node.placement else {
            return None;
        };
        self.routes
            .get(&placement.device)?
            .get(&placement.route_device)
    }

    pub(super) fn effective_levels<'graph>(
        &'graph self,
        node: &'graph AudioNode,
    ) -> &'graph Levels {
        self.active_route(node)
            .map_or(&node.levels, |route| &route.levels)
    }

    fn channel_levels(&self, direction: Direction) -> DefaultChannel {
        let Some(DefaultNode { node, .. }) = self.default_node(direction) else {
            return DefaultChannel::Unresolved;
        };
        let levels = self.effective_levels(node);
        let percent = levels.percent();
        DefaultChannel::Resolved(ChannelLevels {
            percent,
            mute_state: if levels.mute_state == MuteState::Muted || percent == 0 {
                MuteState::Muted
            } else {
                MuteState::Audible
            },
            device: node.description.clone(),
        })
    }

    fn resolved_output_kind(&self, node: &AudioNode) -> OutputKind {
        if let Some(route) = self.active_route(node)
            && let KindHint::Hinted(port_kind) = route.port_kind
        {
            return port_kind;
        }
        if let KindHint::Hinted(props_kind) = node.props_kind {
            return props_kind;
        }
        if let NodeDevice::Device(device_id) = node.device
            && let Some(device_kind) = self.device_kinds.get(&device_id)
        {
            return *device_kind;
        }
        OutputKind::from(node.name.as_str())
    }

    fn window_audio(&self) -> HashMap<i32, WindowAudio> {
        let mut streams_by_pid: HashMap<i32, Vec<&AppStream>> = HashMap::new();
        for stream in self.streams.values() {
            let StreamProcess::Process(pid) = stream.process else {
                continue;
            };
            streams_by_pid.entry(pid).or_default().push(stream);
        }
        streams_by_pid
            .into_iter()
            .map(|(pid, window_streams)| {
                let window_audio = if window_streams
                    .iter()
                    .all(|stream| stream.levels.mute_state == MuteState::Muted)
                {
                    WindowAudio::Muted
                } else if window_streams
                    .iter()
                    .any(|stream| stream.stream_activity == StreamActivity::Running)
                {
                    WindowAudio::Audible
                } else {
                    WindowAudio::Silent
                };
                (pid, window_audio)
            })
            .collect()
    }

    pub(super) fn audio_state(&self) -> AudioState {
        AudioState {
            sink: self.channel_levels(Direction::Output),
            source: self.channel_levels(Direction::Input),
            output_kind: self
                .default_node(Direction::Output)
                .map(|default_sink| self.resolved_output_kind(default_sink.node))
                .unwrap_or_default(),
            windows: Arc::new(self.window_audio()),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DefaultNodeChange {
    Named { direction: Direction, name: String },
    Cleared { direction: Direction },
}

#[derive(Debug, PartialEq)]
pub(super) struct RouteLevels {
    pub(super) index: u32,
    pub(super) route_device: u32,
    pub(super) port_kind: KindHint,
    pub(super) update: LevelsUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum StreamActivity {
    Running,
    #[default]
    Idle,
}

#[cfg(test)]
mod audio_graph_state_tests;
