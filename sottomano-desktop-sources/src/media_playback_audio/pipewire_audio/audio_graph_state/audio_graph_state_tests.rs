use super::*;
use crate::media_playback_audio::pipewire_audio::{
    graph_proxy_listeners::default_node_change,
    level_command_writes::{LevelChange, cubic},
};

fn resolved_percent(channel: &DefaultChannel) -> i64 {
    let DefaultChannel::Resolved(levels) = channel else {
        panic!("the default channel resolves to a node");
    };
    levels.percent
}

fn node(direction: Direction, name: &str, volumes: &[f32]) -> AudioNode {
    AudioNode {
        direction,
        name: name.to_owned(),
        description: DeviceDescription::Described(name.to_owned()),
        device: NodeDevice::Deviceless,
        props_kind: KindHint::Unhinted,
        serial: 0,
        placement: NodePlacement::Unrouted,
        levels: Levels {
            channel_volumes: volumes.to_vec(),
            mute_state: MuteState::Audible,
        },
    }
}

#[test]
fn sample_prefers_route_over_node_props() {
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
            port_kind: KindHint::Unhinted,
            update: LevelsUpdate {
                changes: vec![LevelChange::Volumes(vec![cubic(29)])],
            },
        },
    );
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Output,
        name: "speaker".to_owned(),
    });

    assert_eq!(resolved_percent(&graph.audio_state().sink), 29);
}

#[test]
fn sample_falls_back_to_node_props_without_route() {
    let mut graph = AudioGraph::default();
    graph.add_node(7, node(Direction::Output, "null-sink", &[cubic(42)]));
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Output,
        name: "null-sink".to_owned(),
    });

    assert_eq!(resolved_percent(&graph.audio_state().sink), 42);
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
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Output,
        name: "bluez".to_owned(),
    });

    assert_eq!(resolved_percent(&graph.audio_state().sink), 70);
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
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Input,
        name: "mic1".to_owned(),
    });

    assert_eq!(resolved_percent(&graph.audio_state().source), 60);

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
            port_kind: KindHint::Unhinted,
            update: LevelsUpdate {
                changes: vec![
                    LevelChange::Volumes(vec![cubic(35)]),
                    LevelChange::Mute(MuteState::Audible),
                ],
            },
        },
    );
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Input,
        name: "bluez_in".to_owned(),
    });

    assert_eq!(resolved_percent(&graph.audio_state().source), 35);

    graph.remove_object(97);

    graph.remove_object(93);

    assert_eq!(graph.audio_state().source, DefaultChannel::Unresolved);

    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Input,
        name: "mic1".to_owned(),
    });

    assert_eq!(resolved_percent(&graph.audio_state().source), 60);
}

#[test]
fn a_bluez_profile_switch_recovers_the_sample() {
    let mut graph = AudioGraph::default();
    graph.add_node(
        97,
        AudioNode {
            serial: 1593,
            ..node(Direction::Input, "bluez_in", &[cubic(35)])
        },
    );
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Input,
        name: "bluez_in".to_owned(),
    });

    assert_eq!(resolved_percent(&graph.audio_state().source), 35);

    graph.remove_object(97);

    assert_eq!(graph.audio_state().source, DefaultChannel::Unresolved);

    graph.add_node(
        101,
        AudioNode {
            serial: 29349,
            ..node(Direction::Input, "bluez_in", &[cubic(35)])
        },
    );

    assert_eq!(resolved_percent(&graph.audio_state().source), 35);
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
            port_kind: KindHint::Unhinted,
            update: LevelsUpdate {
                changes: vec![LevelChange::Volumes(vec![cubic(29)])],
            },
        },
    );
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Output,
        name: "speaker".to_owned(),
    });

    graph.remove_object(46);

    assert_eq!(resolved_percent(&graph.audio_state().sink), 54);
}

#[test]
fn a_metadata_clear_never_forgets_the_default() {
    let mut graph = AudioGraph::default();
    graph.add_node(56, node(Direction::Output, "speaker", &[cubic(54)]));
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Output,
        name: "speaker".to_owned(),
    });

    assert!(default_node_change(None, None).is_none());
    assert_eq!(resolved_percent(&graph.audio_state().sink), 54);
    graph.remove_object(56);

    assert_eq!(graph.audio_state().sink, DefaultChannel::Unresolved);
}

fn bluez_headset_graph() -> AudioGraph {
    let mut graph = AudioGraph::default();
    graph.add_node(
        97,
        AudioNode {
            device: NodeDevice::Device(93),
            ..node(Direction::Output, "bluez_output.00_11_22.1", &[cubic(40)])
        },
    );
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Output,
        name: "bluez_output.00_11_22.1".to_owned(),
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
            device: NodeDevice::Device(46),
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
            port_kind: KindHint::Hinted(OutputKind::Headphones),
            update: LevelsUpdate::default(),
        },
    );
    graph.set_default(DefaultNodeChange::Named {
        direction: Direction::Output,
        name: "alsa_output.analog-stereo".to_owned(),
    });

    assert_eq!(graph.audio_state().output_kind, OutputKind::Headphones);
}
