use std::{collections::HashMap, sync::Arc};

use super::{AudioState, ChannelLevels, Direction, MuteState, OutputKind, WindowAudio};

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
        if let Some(volumes) = update.channel_volumes {
            self.channel_volumes = volumes;
        }
        if let Some(mute_state) = update.mute_state {
            self.mute_state = mute_state;
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct LevelsUpdate {
    pub(super) channel_volumes: Option<Vec<f32>>,
    pub(super) mute_state: Option<MuteState>,
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
    pub(super) description: Option<String>,
    pub(super) device_id: Option<u32>,
    pub(super) props_kind: Option<OutputKind>,
    pub(super) serial: u64,
    pub(super) placement: Option<RoutePlacement>,
    pub(super) levels: Levels,
}

#[derive(Debug)]
pub(super) struct Route {
    pub(super) index: u32,
    pub(super) port_kind: Option<OutputKind>,
    pub(super) levels: Levels,
}

#[derive(Debug)]
pub(super) struct AppStream {
    pub(super) pid: Option<i32>,
    pub(super) stream_activity: StreamActivity,
    pub(super) levels: Levels,
}

#[derive(Debug, Default)]
pub(super) struct AudioGraph {
    pub(super) nodes: HashMap<u32, AudioNode>,
    pub(super) streams: HashMap<u32, AppStream>,
    routes: HashMap<u32, HashMap<u32, Route>>,
    device_kinds: HashMap<u32, OutputKind>,
    default_sink: Option<String>,
    default_source: Option<String>,
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
            node.placement = Some(placement);
        }
    }

    pub(super) fn set_kind_hint(&mut self, id: u32, kind: OutputKind) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.props_kind = Some(kind);
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
        pid: Option<i32>,
        stream_activity: StreamActivity,
    ) {
        if let Some(stream) = self.streams.get_mut(&id) {
            if pid.is_some() {
                stream.pid = pid;
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
                port_kind: None,
                levels: Levels::default(),
            });
        entry.index = route.index;
        entry.port_kind = route.port_kind;
        entry.levels.merge(route.update);
    }

    pub(super) fn set_default(&mut self, change: DefaultNodeChange) {
        match change.direction {
            Direction::Output => self.default_sink = change.name,
            Direction::Input => self.default_source = change.name,
        }
    }

    pub(super) fn remove_object(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.streams.remove(&id);
        self.routes.remove(&id);
        self.device_kinds.remove(&id);
    }

    pub(super) fn default_name(&self, direction: Direction) -> Option<&str> {
        match direction {
            Direction::Output => self.default_sink.as_deref(),
            Direction::Input => self.default_source.as_deref(),
        }
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
        let placement = node.placement.as_ref()?;
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

    fn channel_levels(&self, direction: Direction) -> Option<ChannelLevels> {
        let DefaultNode { node, .. } = self.default_node(direction)?;
        let levels = self.effective_levels(node);
        let percent = levels.percent();
        Some(ChannelLevels {
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
        self.active_route(node)
            .and_then(|route| route.port_kind)
            .or(node.props_kind)
            .or_else(|| self.device_kinds.get(&node.device_id?).copied())
            .unwrap_or_else(|| OutputKind::from(node.name.as_str()))
    }

    fn window_audio(&self) -> HashMap<i32, WindowAudio> {
        let mut streams_by_pid: HashMap<i32, Vec<&AppStream>> = HashMap::new();
        for stream in self.streams.values() {
            let Some(pid) = stream.pid else { continue };
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
pub(super) struct DefaultNodeChange {
    pub(super) direction: Direction,
    pub(super) name: Option<String>,
}

#[derive(Debug, PartialEq)]
pub(super) struct RouteLevels {
    pub(super) index: u32,
    pub(super) route_device: u32,
    pub(super) port_kind: Option<OutputKind>,
    pub(super) update: LevelsUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum StreamActivity {
    Running,
    #[default]
    Idle,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_playback_audio::pipewire_audio::{
        graph_proxy_listeners::default_node_change, level_command_writes::cubic,
    };

    fn node(direction: Direction, name: &str, volumes: &[f32]) -> AudioNode {
        AudioNode {
            direction,
            name: name.to_owned(),
            description: Some(name.to_owned()),
            device_id: None,
            props_kind: None,
            serial: 0,
            placement: None,
            levels: Levels {
                channel_volumes: volumes.to_vec(),
                mute_state: MuteState::Audible,
            },
        }
    }

    #[test]
    fn reading_prefers_route_over_node_props() {
        let mut graph = AudioGraph::default();
        graph.add_node(56, node(Direction::Output, "speaker", &[cubic(54)]));
        graph.set_placement(
            56,
            RoutePlacement {
                device: 46,
                route_device: 3,
            },
        );
        graph.merge_route_levels(
            46,
            RouteLevels {
                index: 3,
                route_device: 3,
                port_kind: None,
                update: LevelsUpdate {
                    channel_volumes: Some(vec![cubic(29)]),
                    mute_state: None,
                },
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Output,
            name: Some("speaker".to_owned()),
        });

        assert_eq!(graph.audio_state().sink.unwrap().percent, 29);
    }

    #[test]
    fn reading_falls_back_to_node_props_without_route() {
        let mut graph = AudioGraph::default();
        graph.add_node(7, node(Direction::Output, "null-sink", &[cubic(42)]));
        graph.set_default(DefaultNodeChange {
            direction: Direction::Output,
            name: Some("null-sink".to_owned()),
        });

        assert_eq!(graph.audio_state().sink.unwrap().percent, 42);
    }

    #[test]
    fn highest_serial_wins_when_names_collide_on_a_recycled_id() {
        let mut graph = AudioGraph::default();
        graph.add_node(
            91,
            AudioNode {
                serial: 1593,
                ..node(Direction::Output, "bluez", &[cubic(10)])
            },
        );
        graph.add_node(
            90,
            AudioNode {
                serial: 29349,
                ..node(Direction::Output, "bluez", &[cubic(70)])
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Output,
            name: Some("bluez".to_owned()),
        });

        assert_eq!(graph.audio_state().sink.unwrap().percent, 70);
    }

    #[test]
    fn a_headset_connect_then_disconnect_round_trips() {
        let mut graph = AudioGraph::default();
        graph.add_node(
            58,
            AudioNode {
                serial: 58,
                ..node(Direction::Input, "mic1", &[cubic(60)])
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Input,
            name: Some("mic1".to_owned()),
        });

        assert_eq!(graph.audio_state().source.unwrap().percent, 60);

        graph.add_node(
            97,
            AudioNode {
                serial: 1593,
                ..node(Direction::Input, "bluez_in", &[])
            },
        );
        graph.set_placement(
            97,
            RoutePlacement {
                device: 93,
                route_device: 0,
            },
        );
        graph.merge_route_levels(
            93,
            RouteLevels {
                index: 0,
                route_device: 0,
                port_kind: None,
                update: LevelsUpdate {
                    channel_volumes: Some(vec![cubic(35)]),
                    mute_state: Some(MuteState::Audible),
                },
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Input,
            name: Some("bluez_in".to_owned()),
        });

        assert_eq!(graph.audio_state().source.unwrap().percent, 35);

        graph.remove_object(97);

        graph.remove_object(93);

        assert!(graph.audio_state().source.is_none());

        graph.set_default(DefaultNodeChange {
            direction: Direction::Input,
            name: Some("mic1".to_owned()),
        });

        assert_eq!(graph.audio_state().source.unwrap().percent, 60);
    }

    #[test]
    fn a_bluez_profile_switch_recovers_the_reading() {
        let mut graph = AudioGraph::default();
        graph.add_node(
            97,
            AudioNode {
                serial: 1593,
                ..node(Direction::Input, "bluez_in", &[cubic(35)])
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Input,
            name: Some("bluez_in".to_owned()),
        });

        assert_eq!(graph.audio_state().source.unwrap().percent, 35);

        graph.remove_object(97);

        assert!(graph.audio_state().source.is_none());

        graph.add_node(
            101,
            AudioNode {
                serial: 29349,
                ..node(Direction::Input, "bluez_in", &[cubic(35)])
            },
        );

        assert_eq!(graph.audio_state().source.unwrap().percent, 35);
    }

    #[test]
    fn removing_a_device_drops_its_routes() {
        let mut graph = AudioGraph::default();
        graph.add_node(56, node(Direction::Output, "speaker", &[cubic(54)]));
        graph.set_placement(
            56,
            RoutePlacement {
                device: 46,
                route_device: 3,
            },
        );
        graph.merge_route_levels(
            46,
            RouteLevels {
                index: 3,
                route_device: 3,
                port_kind: None,
                update: LevelsUpdate {
                    channel_volumes: Some(vec![cubic(29)]),
                    mute_state: None,
                },
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Output,
            name: Some("speaker".to_owned()),
        });

        graph.remove_object(46);

        assert_eq!(graph.audio_state().sink.unwrap().percent, 54);
    }

    #[test]
    fn a_metadata_clear_never_forgets_the_default() {
        let mut graph = AudioGraph::default();
        graph.add_node(56, node(Direction::Output, "speaker", &[cubic(54)]));
        graph.set_default(DefaultNodeChange {
            direction: Direction::Output,
            name: Some("speaker".to_owned()),
        });

        assert!(default_node_change(None, None).is_none());
        assert_eq!(graph.audio_state().sink.unwrap().percent, 54);
        graph.remove_object(56);

        assert!(graph.audio_state().sink.is_none());
    }

    fn bluez_headset_graph() -> AudioGraph {
        let mut graph = AudioGraph::default();
        graph.add_node(
            97,
            AudioNode {
                device_id: Some(93),
                ..node(Direction::Output, "bluez_output.00_11_22.1", &[cubic(40)])
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Output,
            name: Some("bluez_output.00_11_22.1".to_owned()),
        });
        graph
    }

    #[test]
    fn a_bluez_sink_takes_the_device_form_factor() {
        let mut graph = bluez_headset_graph();
        assert_eq!(graph.audio_state().output_kind, OutputKind::Speaker);
        graph.set_device_kind(93, OutputKind::Headset);

        assert_eq!(graph.audio_state().output_kind, OutputKind::Headset);
        graph.remove_object(93);

        assert_eq!(graph.audio_state().output_kind, OutputKind::Speaker);
    }

    #[test]
    fn the_active_port_beats_device_and_node_name() {
        let mut graph = AudioGraph::default();
        graph.add_node(
            56,
            AudioNode {
                device_id: Some(46),
                ..node(Direction::Output, "alsa_output.analog-stereo", &[cubic(54)])
            },
        );
        graph.set_device_kind(46, OutputKind::Speaker);
        graph.set_placement(
            56,
            RoutePlacement {
                device: 46,
                route_device: 3,
            },
        );
        graph.merge_route_levels(
            46,
            RouteLevels {
                index: 2,
                route_device: 3,
                port_kind: Some(OutputKind::Headphones),
                update: LevelsUpdate::default(),
            },
        );
        graph.set_default(DefaultNodeChange {
            direction: Direction::Output,
            name: Some("alsa_output.analog-stereo".to_owned()),
        });

        assert_eq!(graph.audio_state().output_kind, OutputKind::Headphones);
    }
}
