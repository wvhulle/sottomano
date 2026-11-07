use pipewire::spa::{param::ParamType, pod::Pod};

use super::{
    CommandKind, Direction, Target,
    audio_graph_state::{DefaultNode, Levels},
    graph_proxy_listeners::BoundAudioSession,
    spa_pod_levels::{level_property, pod_bytes, props_object, route_object},
};
use crate::media_playback_audio::pipewire_audio::MuteState;

pub(super) enum LevelChange {
    Volumes(Vec<f32>),
    Mute(MuteState),
}

struct Resolved {
    pub(super) change: LevelChange,
    pub(super) percent: i64,
}

fn resolve(levels: &Levels, kind: CommandKind) -> Option<Resolved> {
    let current = levels.percent();
    let channels = levels.channel_volumes.len();
    let uniform = |percent: i64| {
        (channels > 0).then(|| Resolved {
            change: LevelChange::Volumes(vec![cubic(percent); channels]),
            percent,
        })
    };
    match kind {
        CommandKind::SetVolume(percent) => uniform(percent),
        CommandKind::AdjustVolume(delta) => uniform((current + delta).clamp(0, 100)),
        CommandKind::ToggleMute => Some(Resolved {
            change: LevelChange::Mute(levels.mute_state.toggled()),
            percent: current,
        }),
    }
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn cubic(percent: i64) -> f32 {
    (percent as f32 / 100.0).powi(3)
}

pub(super) fn apply_level_command(
    session: &BoundAudioSession,
    target: Target,
    kind: CommandKind,
) -> Option<i64> {
    match target {
        Target::DefaultSink => apply_to_default(session, Direction::Output, kind),
        Target::DefaultSource => apply_to_default(session, Direction::Input, kind),
        Target::AppStreams(pid) => apply_to_streams(session, pid, kind),
    }
}

fn apply_to_default(
    session: &BoundAudioSession,
    direction: Direction,
    kind: CommandKind,
) -> Option<i64> {
    let DefaultNode { id, node } = session.graph.default_node(direction)?;
    let resolved = resolve(session.graph.effective_levels(node), kind)?;
    write_level_change(session, id, &resolved.change)?;
    Some(resolved.percent)
}

fn apply_to_streams(session: &BoundAudioSession, pid: i32, kind: CommandKind) -> Option<i64> {
    let app_streams: Vec<_> = session
        .graph
        .streams
        .iter()
        .filter(|(_, stream)| stream.pid == Some(pid))
        .collect();
    let app_mute_state = if !app_streams.is_empty()
        && app_streams
            .iter()
            .all(|(_, stream)| stream.levels.mute_state == MuteState::Muted)
    {
        MuteState::Muted
    } else {
        MuteState::Audible
    };
    app_streams
        .iter()
        .filter_map(|(id, stream)| {
            let resolved = match kind {
                CommandKind::ToggleMute => Resolved {
                    change: LevelChange::Mute(app_mute_state.toggled()),
                    percent: stream.levels.percent(),
                },
                CommandKind::SetVolume(_) | CommandKind::AdjustVolume(_) => {
                    resolve(&stream.levels, kind)?
                }
            };
            write_level_change(session, **id, &resolved.change)?;
            Some(resolved.percent)
        })
        .reduce(|_, percent| percent)
}

fn write_level_change(session: &BoundAudioSession, id: u32, change: &LevelChange) -> Option<()> {
    write_to_route(session, id, change).or_else(|| write_to_node_props(session, id, change))
}

fn write_to_route(session: &BoundAudioSession, id: u32, change: &LevelChange) -> Option<()> {
    let node = session.graph.nodes.get(&id)?;
    let placement = node.placement.as_ref()?;
    let route = session.graph.active_route(node)?;
    let device = &session.proxies.devices.get(&placement.device)?.proxy;
    let bytes = pod_bytes(&route_object(route.index, placement.route_device, change))?;
    device.set_param(ParamType::Route, 0, Pod::from_bytes(&bytes)?);
    Some(())
}

fn write_to_node_props(session: &BoundAudioSession, id: u32, change: &LevelChange) -> Option<()> {
    let node = &session.proxies.nodes.get(&id)?.proxy;
    let bytes = pod_bytes(&props_object(vec![level_property(change)]))?;
    node.set_param(ParamType::Props, 0, Pod::from_bytes(&bytes)?);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_playback_audio::pipewire_audio::MuteState;

    #[test]
    fn adjust_resolves_against_current_level() {
        let levels = Levels {
            channel_volumes: vec![cubic(33), cubic(33)],
            mute_state: MuteState::Audible,
        };
        let resolved = resolve(&levels, CommandKind::AdjustVolume(5)).unwrap();
        assert_eq!(resolved.percent, 38);
        match resolved.change {
            LevelChange::Volumes(volumes) => assert_eq!(volumes, vec![cubic(38); 2]),
            LevelChange::Mute(_) => panic!("expected a volume change"),
        }
    }

    #[test]
    fn adjust_clamps_at_the_ceiling() {
        let levels = Levels {
            channel_volumes: vec![cubic(98)],
            mute_state: MuteState::Audible,
        };
        assert_eq!(
            resolve(&levels, CommandKind::AdjustVolume(5))
                .unwrap()
                .percent,
            100
        );
    }
}
