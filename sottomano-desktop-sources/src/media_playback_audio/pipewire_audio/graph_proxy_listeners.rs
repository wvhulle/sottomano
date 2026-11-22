use std::collections::HashMap;

use pipewire::{
    device::{Device, DeviceChangeMask, DeviceInfoRef, DeviceListener},
    metadata::{Metadata, MetadataListener},
    node::{Node, NodeInfoRef, NodeListener, NodeState},
    proxy::ProxyT,
    registry::{GlobalObject, Registry},
    spa::{param::ParamType, utils::dict::DictRef},
    types::ObjectType,
};

use super::{
    Direction, OutputKind,
    audio_graph_state::{DefaultNodeChange, StreamActivity, StreamProcess},
    live_audio_session::LiveAudioGraph,
    spa_pod_levels::{
        default_node_name, node_direction, node_from_props, parse_props, parse_route,
        route_placement, stream_from_props, stream_process,
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
            tracing::warn!("audio: bind {label} {} failed: {bind_error}", global.id);
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
                if let Some(kind) = OutputKind::described_props_kind(props, "device.icon_name") {
                    live_graph.update(|graph| graph.set_kind_hint(id, kind));
                }
            }
        };
        if let Some(bound_node) = bind_levelled_node(registry, live_graph, global, "node", on_info)
        {
            live_graph.update(|graph| graph.add_node(id, node));
            keep_node_proxy(live_graph, id, bound_node);
        }
    } else if props.get("media.class") == Some("Stream/Output/Audio") {
        let on_info = {
            let live_graph = live_graph.clone();
            move |info: &NodeInfoRef| {
                let process = info
                    .props()
                    .map_or(StreamProcess::Unidentified, stream_process);
                let stream_activity = match info.state() {
                    NodeState::Running => StreamActivity::Running,
                    _ => StreamActivity::Idle,
                };
                live_graph.update(|graph| graph.set_stream_info(id, process, stream_activity));
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
                    && let Some(kind) = info.props().and_then(|props| {
                        OutputKind::described_props_kind(props, "device.icon-name")
                    })
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
    Some(match value.and_then(default_node_name) {
        Some(name) => DefaultNodeChange::Named { direction, name },
        None => DefaultNodeChange::Cleared { direction },
    })
}
