use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use pipewire::{
    core::CoreRc,
    device::{Device, DeviceChangeMask, DeviceInfoRef, DeviceListener},
    metadata::{Metadata, MetadataListener},
    node::{Node, NodeInfoRef, NodeListener, NodeState},
    proxy::ProxyT,
    registry::{GlobalObject, Registry},
    spa::{
        param::ParamType,
        utils::{dict::DictRef, result::AsyncSeq},
    },
    types::ObjectType,
};
use tokio::sync::watch;

use super::{
    AudioState, Direction, OutputKind,
    audio_graph_state::{AudioGraph, DefaultNodeChange},
    spa_pod_levels::{
        default_node_name, is_output_stream, node_direction, node_from_props, parse_props,
        parse_route, route_placement, stream_from_props, stream_pid,
    },
};

pub(super) struct Bound<Proxy, Listener> {
    pub(super) proxy: Proxy,
    _listener: Listener,
}

#[derive(Default)]
pub(super) struct Proxies {
    pub(super) nodes: HashMap<u32, Bound<Node, NodeListener>>,
    pub(super) devices: HashMap<u32, Bound<Device, DeviceListener>>,
    defaults: Vec<Bound<Metadata, MetadataListener>>,
}

#[derive(Clone, Copy, Default)]
pub(super) enum GraphEnumeration {
    #[default]
    NotStarted,
    ListingGlobals(AsyncSeq),
    AnsweringBinds(AsyncSeq),
    Settled,
}

impl GraphEnumeration {
    pub(super) fn started(sync_result: Result<AsyncSeq, pipewire::Error>) -> Self {
        sync_result
            .inspect_err(|sync_error| tracing::warn!("audio: pipewire sync failed: {sync_error}"))
            .map_or(Self::Settled, Self::ListingGlobals)
    }

    fn advanced(
        self,
        finished: AsyncSeq,
        sync_again: impl FnOnce() -> Result<AsyncSeq, pipewire::Error>,
    ) -> Self {
        match self {
            Self::ListingGlobals(pending) if pending == finished => sync_again()
                .inspect_err(|sync_error| {
                    tracing::warn!("audio: pipewire sync failed: {sync_error}");
                })
                .map_or(Self::Settled, Self::AnsweringBinds),
            Self::AnsweringBinds(pending) if pending == finished => Self::Settled,
            unfinished => unfinished,
        }
    }
}

#[derive(Default)]
pub(super) struct BoundAudioSession {
    pub(super) graph: AudioGraph,
    pub(super) proxies: Proxies,
    pub(super) enumeration: GraphEnumeration,
    reported_missing: HashSet<Direction>,
}

fn report_missing_defaults(session: &mut BoundAudioSession) {
    if !matches!(session.enumeration, GraphEnumeration::Settled) {
        return;
    }
    for direction in [Direction::Output, Direction::Input] {
        let graph = &session.graph;
        let present = graph.node_count(direction);
        if graph.default_node(direction).is_some() || present == 0 {
            session.reported_missing.remove(&direction);
        } else if session.reported_missing.insert(direction) {
            let label = direction.label();
            if let Some(name) = graph.default_name(direction) {
                tracing::warn!(
                    "audio: default {label} {name} matches none of the {present} {label} nodes"
                );
            } else {
                tracing::warn!(
                    "audio: no default {label} has been published while {present} {label} nodes \
                     are present"
                );
            }
        }
    }
}

fn report_unresolved(previous: &AudioState, next: &AudioState, graph: &AudioGraph) {
    for direction in [Direction::Output, Direction::Input] {
        if previous.channel(direction).is_some() && next.channel(direction).is_none() {
            tracing::warn!(
                "audio: default {} {:?} no longer resolves to a node",
                direction.label(),
                graph.default_name(direction)
            );
        }
    }
}

#[derive(Clone)]
pub(super) struct LiveAudioGraph {
    pub(super) session: Rc<RefCell<BoundAudioSession>>,
    pub(super) states: watch::Sender<AudioState>,
}

impl LiveAudioGraph {
    pub(super) fn new(states: watch::Sender<AudioState>) -> Self {
        Self {
            session: Rc::default(),
            states,
        }
    }

    pub(super) fn update(&self, change: impl FnOnce(&mut AudioGraph)) {
        let mut session = self.session.borrow_mut();
        change(&mut session.graph);
        let next = session.graph.audio_state();
        self.states.send_if_modified(|current| {
            if *current == next {
                return false;
            }
            report_unresolved(current, &next, &session.graph);
            *current = next;
            true
        });
        report_missing_defaults(&mut session);
    }

    pub(super) fn settle_enumeration(&self, finished: AsyncSeq, core: &CoreRc) {
        let mut session = self.session.borrow_mut();
        session.enumeration = session.enumeration.advanced(finished, || core.sync(0));
        report_missing_defaults(&mut session);
    }

    pub(super) fn remove_object(&self, id: u32) {
        {
            let mut session = self.session.borrow_mut();
            session.proxies.nodes.remove(&id);
            session.proxies.devices.remove(&id);
        }
        self.update(|graph| graph.remove_object(id));
    }
}

pub(super) fn track_global(
    registry: &Registry,
    live_graph: &LiveAudioGraph,
    global: &GlobalObject<&DictRef>,
) {
    let Some(props) = global.props else {
        return;
    };
    match global.type_ {
        ObjectType::Node => track_node(registry, live_graph, global, props),
        ObjectType::Device => track_device(registry, live_graph, global, props),
        ObjectType::Metadata => track_defaults(registry, live_graph, global, props),
        _ => {}
    }
}

fn bind<Proxy: ProxyT>(
    registry: &Registry,
    global: &GlobalObject<&DictRef>,
    label: &str,
) -> Option<Proxy> {
    registry
        .bind::<Proxy, _>(global)
        .inspect_err(|bind_error| {
            tracing::warn!("audio: bind {label} {} failed: {bind_error}", global.id)
        })
        .ok()
}

fn track_node(
    registry: &Registry,
    live_graph: &LiveAudioGraph,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    let id = global.id;
    if let Some(direction) = node_direction(props) {
        let Some(node) = node_from_props(props, direction) else {
            return;
        };
        let on_info = {
            let live_graph = live_graph.clone();
            move |info: &NodeInfoRef| {
                let Some(props) = info.props() else {
                    return;
                };
                if let Some(placement) = route_placement(props) {
                    live_graph.update(|graph| graph.set_placement(id, placement));
                }
                if let Some(kind) = OutputKind::of_described_props(props, "device.icon_name") {
                    live_graph.update(|graph| graph.set_kind_hint(id, kind));
                }
            }
        };
        if let Some(bound_node) = bind_levelled_node(registry, live_graph, global, "node", on_info)
        {
            live_graph.update(|graph| graph.add_node(id, node));
            keep_node_proxy(live_graph, id, bound_node);
        }
    } else if is_output_stream(props) {
        let on_info = {
            let live_graph = live_graph.clone();
            move |info: &NodeInfoRef| {
                let pid = info.props().and_then(stream_pid);
                let running = matches!(info.state(), NodeState::Running);
                live_graph.update(|graph| graph.set_stream_info(id, pid, running));
            }
        };
        if let Some(bound_stream) =
            bind_levelled_node(registry, live_graph, global, "stream", on_info)
        {
            live_graph.update(|graph| graph.add_stream(id, stream_from_props(props)));
            keep_node_proxy(live_graph, id, bound_stream);
        }
    }
}

fn keep_node_proxy(live_graph: &LiveAudioGraph, id: u32, bound_node: Bound<Node, NodeListener>) {
    live_graph
        .session
        .borrow_mut()
        .proxies
        .nodes
        .insert(id, bound_node);
}

fn bind_levelled_node(
    registry: &Registry,
    live_graph: &LiveAudioGraph,
    global: &GlobalObject<&DictRef>,
    label: &str,
    on_info: impl Fn(&NodeInfoRef) + 'static,
) -> Option<Bound<Node, NodeListener>> {
    let proxy = bind::<Node>(registry, global, label)?;
    proxy.subscribe_params(&[ParamType::Props]);
    let id = global.id;
    let live_graph = live_graph.clone();
    let listener = proxy
        .add_listener_local()
        .info(on_info)
        .param(move |_, _, _, _, param| {
            if let Some(update) = param.and_then(parse_props) {
                live_graph.update(|graph| graph.merge_levels(id, update));
            }
        })
        .register();
    Some(Bound {
        proxy,
        _listener: listener,
    })
}

fn track_device(
    registry: &Registry,
    live_graph: &LiveAudioGraph,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    if props.get("media.class") != Some("Audio/Device") {
        return;
    }
    let Some(proxy) = bind::<Device>(registry, global, "device") else {
        return;
    };
    proxy.subscribe_params(&[ParamType::Route]);
    let id = global.id;
    let listener = proxy
        .add_listener_local()
        .info({
            let live_graph = live_graph.clone();
            move |info: &DeviceInfoRef| {
                if info.change_mask().contains(DeviceChangeMask::PROPS)
                    && let Some(kind) = info
                        .props()
                        .and_then(|props| OutputKind::of_described_props(props, "device.icon-name"))
                {
                    live_graph.update(|graph| graph.set_device_kind(id, kind));
                }
                if info.change_mask().contains(DeviceChangeMask::PARAMS)
                    && let Some(device) = live_graph.session.borrow().proxies.devices.get(&id)
                {
                    device
                        .proxy
                        .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
                }
            }
        })
        .param({
            let live_graph = live_graph.clone();
            move |_, param_type, _, _, param| {
                if param_type != ParamType::Route {
                    return;
                }
                if let Some(route) = param.and_then(parse_route) {
                    live_graph.update(|graph| graph.merge_route_levels(id, route));
                }
            }
        })
        .register();
    proxy.enum_params(0, Some(ParamType::Route), 0, u32::MAX);
    live_graph.session.borrow_mut().proxies.devices.insert(
        id,
        Bound {
            proxy,
            _listener: listener,
        },
    );
}

fn track_defaults(
    registry: &Registry,
    live_graph: &LiveAudioGraph,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    if props.get("metadata.name") != Some("default") {
        return;
    }
    let Some(proxy) = bind::<Metadata>(registry, global, "default metadata") else {
        return;
    };
    let listener = proxy
        .add_listener_local()
        .property({
            let live_graph = live_graph.clone();
            move |_, key, _, value| {
                if let Some(change) = default_node_change(key, value) {
                    live_graph.update(|graph| graph.set_default(change));
                }
                0
            }
        })
        .register();
    live_graph
        .session
        .borrow_mut()
        .proxies
        .defaults
        .push(Bound {
            proxy,
            _listener: listener,
        });
}

pub(super) fn default_node_change(
    key: Option<&str>,
    value: Option<&str>,
) -> Option<DefaultNodeChange> {
    let direction = match key? {
        "default.audio.sink" => Direction::Output,
        "default.audio.source" => Direction::Input,
        _ => return None,
    };
    Some(DefaultNodeChange {
        direction,
        name: value.and_then(default_node_name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_playback_audio::pipewire_audio::audio_graph_state::{AudioNode, Levels};

    fn session_with_undefaulted_sink(enumeration: GraphEnumeration) -> BoundAudioSession {
        let mut graph = AudioGraph::default();
        graph.add_node(
            56,
            AudioNode {
                direction: Direction::Output,
                name: "speaker".to_owned(),
                description: None,
                device_id: None,
                props_kind: None,
                serial: 0,
                placement: None,
                levels: Levels::default(),
            },
        );
        BoundAudioSession {
            graph,
            enumeration,
            ..BoundAudioSession::default()
        }
    }

    #[test]
    fn missing_defaults_stay_unreported_while_enumerating() {
        let mut session = session_with_undefaulted_sink(GraphEnumeration::NotStarted);
        report_missing_defaults(&mut session);
        assert!(session.reported_missing.is_empty());
    }

    #[test]
    fn missing_defaults_are_reported_once_settled() {
        let mut session = session_with_undefaulted_sink(GraphEnumeration::Settled);
        report_missing_defaults(&mut session);
        assert!(session.reported_missing.contains(&Direction::Output));
    }

    #[test]
    fn enumeration_settles_after_two_matching_sync_rounds() {
        let first_sync = AsyncSeq::from_seq(1);
        let second_sync = AsyncSeq::from_seq(2);
        let unrelated_sync = AsyncSeq::from_seq(9);
        let listing = GraphEnumeration::started(Ok(first_sync));
        let unchanged = listing.advanced(unrelated_sync, || Ok(second_sync));
        assert!(
            matches!(unchanged, GraphEnumeration::ListingGlobals(pending) if pending == first_sync)
        );
        let answering = unchanged.advanced(first_sync, || Ok(second_sync));
        assert!(
            matches!(answering, GraphEnumeration::AnsweringBinds(pending) if pending == second_sync)
        );
        let still_answering = answering.advanced(first_sync, || Ok(unrelated_sync));
        assert!(matches!(
            still_answering,
            GraphEnumeration::AnsweringBinds(_)
        ));
        assert!(matches!(
            still_answering.advanced(second_sync, || Ok(unrelated_sync)),
            GraphEnumeration::Settled
        ));
    }
}
