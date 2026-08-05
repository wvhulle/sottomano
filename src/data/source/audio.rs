use std::{
    cell::RefCell,
    collections::HashMap,
    fmt, io,
    rc::Rc,
    sync::mpsc::{SyncSender, sync_channel},
    thread,
    time::{Duration, Instant},
};

use iced::{
    Subscription,
    futures::{SinkExt, channel::mpsc::Sender},
    stream::channel,
};
use pipewire::{
    channel::{Receiver as CommandReceiver, Sender as CommandSender, channel as command_channel},
    context::Context,
    device::{Device, DeviceChangeMask, DeviceListener},
    main_loop::MainLoop,
    metadata::{Metadata, MetadataListener},
    node::{Node, NodeListener},
    registry::{GlobalObject, Registry},
    spa::{
        param::ParamType,
        pod::{
            Object, Pod, Property, PropertyFlags, Value, ValueArray, deserialize::PodDeserializer,
            serialize::PodSerializer,
        },
        sys::{
            SPA_PARAM_Props, SPA_PARAM_ROUTE_device, SPA_PARAM_ROUTE_index, SPA_PARAM_ROUTE_props,
            SPA_PARAM_ROUTE_save, SPA_PARAM_Route, SPA_PROP_channelVolumes, SPA_PROP_mute,
            SPA_TYPE_OBJECT_ParamRoute, SPA_TYPE_OBJECT_Props,
        },
        utils::dict::DictRef,
    },
    types::ObjectType,
};
use tokio::sync::watch;

const REPLY_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Debug, Clone)]
pub struct Reading {
    pub percent: i64,
    pub muted: bool,
    pub device: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AudioState {
    pub sink: Option<Reading>,
    pub source: Option<Reading>,
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    Connected(Commands),
    State(AudioState),
}

#[derive(Debug, Clone, Copy)]
pub enum Target {
    DefaultSink,
    DefaultSource,
}

#[derive(Clone)]
pub struct Commands {
    sender: CommandSender<Command>,
}

impl fmt::Debug for Commands {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Commands")
    }
}

impl Commands {
    pub fn set_volume(&self, target: Target, percent: i64) -> Option<i64> {
        self.request(target, CommandKind::SetVolume(percent.clamp(0, 100)))
    }

    pub fn adjust_volume(&self, target: Target, delta: i64) -> Option<i64> {
        self.request(target, CommandKind::AdjustVolume(delta))
    }

    pub fn toggle_mute(&self, target: Target) -> Option<i64> {
        self.request(target, CommandKind::ToggleMute)
    }

    fn request(&self, target: Target, kind: CommandKind) -> Option<i64> {
        let (reply, outcome) = sync_channel(1);
        self.sender
            .send(Command {
                target,
                kind,
                reply,
                expires_at: Instant::now() + REPLY_TIMEOUT,
            })
            .ok()?;
        outcome.recv_timeout(REPLY_TIMEOUT).ok()?
    }
}

pub fn subscription() -> Subscription<AudioEvent> {
    Subscription::run_with("audio", |_| {
        channel(16, move |mut output: Sender<AudioEvent>| async move {
            let (states, mut updates) = watch::channel(AudioState::default());
            let (commands, requests) = command_channel::<Command>();
            if thread::Builder::new()
                .name("pipewire-audio".to_owned())
                .spawn(move || run(&states, requests))
                .is_err()
            {
                return;
            }
            let connected = AudioEvent::Connected(Commands { sender: commands });
            if output.send(connected).await.is_err() {
                return;
            }
            while updates.changed().await.is_ok() {
                let state = updates.borrow_and_update().clone();
                if output.send(AudioEvent::State(state)).await.is_err() {
                    break;
                }
            }
        })
    })
}

struct Command {
    target: Target,
    kind: CommandKind,
    reply: SyncSender<Option<i64>>,
    expires_at: Instant,
}

#[derive(Debug, Clone, Copy)]
enum CommandKind {
    SetVolume(i64),
    AdjustVolume(i64),
    ToggleMute,
}

struct NodeRoute {
    device: u32,
    route_device: u32,
}

struct NodeState {
    name: String,
    description: String,
    route: Option<NodeRoute>,
    channel_volumes: Vec<f32>,
    mute: bool,
    proxy: Node,
    _listener: NodeListener,
}

struct RouteState {
    index: u32,
    device: u32,
    channel_volumes: Vec<f32>,
    mute: bool,
}

struct DeviceState {
    proxy: Device,
    routes: HashMap<u32, RouteState>,
    _listener: DeviceListener,
}

struct Defaults {
    _proxy: Metadata,
    _listener: MetadataListener,
}

#[derive(Default)]
struct Graph {
    nodes: HashMap<u32, NodeState>,
    devices: HashMap<u32, DeviceState>,
    defaults: Vec<Defaults>,
    default_sink: Option<String>,
    default_source: Option<String>,
}

fn run(states: &watch::Sender<AudioState>, commands: CommandReceiver<Command>) {
    pipewire::init();
    let states = Rc::new(states.clone());
    serve(&states, commands);
    states.send_replace(AudioState::default());
}

fn serve(states: &Rc<watch::Sender<AudioState>>, commands: CommandReceiver<Command>) {
    let Ok(main_loop) = MainLoop::new(None) else {
        return;
    };
    let Ok(context) = Context::new(&main_loop) else {
        return;
    };
    let Ok(core) = context.connect(None) else {
        return;
    };
    let Ok(registry) = core.get_registry() else {
        return;
    };
    let registry = Rc::new(registry);
    let graph = Rc::new(RefCell::new(Graph::default()));

    let _attached = commands.attach(main_loop.loop_(), {
        let graph = graph.clone();
        move |command: Command| {
            let outcome = (Instant::now() <= command.expires_at)
                .then(|| apply(&graph.borrow(), command.target, command.kind))
                .flatten();
            let _ = command.reply.send(outcome);
        }
    });

    let _core_listener = core
        .add_listener_local()
        .error({
            let main_loop = main_loop.clone();
            move |_, _, _, _| main_loop.quit()
        })
        .register();

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let registry = registry.clone();
            let graph = graph.clone();
            let states = states.clone();
            move |global| add_global(&registry, &graph, &states, global)
        })
        .global_remove({
            let graph = graph.clone();
            let states = states.clone();
            move |id| {
                {
                    let mut graph = graph.borrow_mut();
                    graph.nodes.remove(&id);
                    graph.devices.remove(&id);
                }
                publish(&graph.borrow(), &states);
            }
        })
        .register();

    main_loop.run();
}

fn add_global(
    registry: &Rc<Registry>,
    graph: &Rc<RefCell<Graph>>,
    states: &Rc<watch::Sender<AudioState>>,
    global: &GlobalObject<&DictRef>,
) {
    let Some(props) = global.props else {
        return;
    };
    match global.type_ {
        ObjectType::Node => add_node(registry, graph, states, global, props),
        ObjectType::Device => add_device(registry, graph, states, global, props),
        ObjectType::Metadata => add_defaults(registry, graph, states, global, props),
        _ => {}
    }
}

fn add_node(
    registry: &Rc<Registry>,
    graph: &Rc<RefCell<Graph>>,
    states: &Rc<watch::Sender<AudioState>>,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    if !matches!(
        props.get("media.class"),
        Some("Audio/Sink" | "Audio/Source")
    ) {
        return;
    }
    let Ok(proxy) = registry.bind::<Node, _>(global) else {
        return;
    };
    proxy.subscribe_params(&[ParamType::Props]);
    let id = global.id;
    let listener = proxy
        .add_listener_local()
        .info({
            let graph = graph.clone();
            let states = states.clone();
            move |info| {
                let Some(route) = info.props().and_then(node_route) else {
                    return;
                };
                {
                    let mut graph = graph.borrow_mut();
                    if let Some(node) = graph.nodes.get_mut(&id) {
                        node.route = Some(route);
                    }
                }
                publish(&graph.borrow(), &states);
            }
        })
        .param({
            let graph = graph.clone();
            let states = states.clone();
            move |_, _, _, _, param| {
                let Some(update) = param.and_then(parse_props) else {
                    return;
                };
                {
                    let mut graph = graph.borrow_mut();
                    if let Some(node) = graph.nodes.get_mut(&id) {
                        if let Some(volumes) = update.volumes {
                            node.channel_volumes = volumes;
                        }
                        if let Some(mute) = update.mute {
                            node.mute = mute;
                        }
                    }
                }
                publish(&graph.borrow(), &states);
            }
        })
        .register();
    graph.borrow_mut().nodes.insert(
        id,
        NodeState {
            name: props.get("node.name").unwrap_or_default().to_owned(),
            description: props
                .get("node.nick")
                .or_else(|| props.get("node.description"))
                .unwrap_or_default()
                .to_owned(),
            route: None,
            channel_volumes: Vec::new(),
            mute: false,
            proxy,
            _listener: listener,
        },
    );
}

fn add_device(
    registry: &Rc<Registry>,
    graph: &Rc<RefCell<Graph>>,
    states: &Rc<watch::Sender<AudioState>>,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    if props.get("media.class") != Some("Audio/Device") {
        return;
    }
    let Ok(proxy) = registry.bind::<Device, _>(global) else {
        return;
    };
    proxy.subscribe_params(&[ParamType::Route]);
    let id = global.id;
    let listener = proxy
        .add_listener_local()
        .info({
            let graph = graph.clone();
            move |info| {
                if !info.change_mask().contains(DeviceChangeMask::PARAMS) {
                    return;
                }
                if let Some(device) = graph.borrow().devices.get(&id) {
                    device
                        .proxy
                        .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
                }
            }
        })
        .param({
            let graph = graph.clone();
            let states = states.clone();
            move |_, param_type, _, _, param| {
                if param_type != ParamType::Route {
                    return;
                }
                let Some(update) = param.and_then(parse_route) else {
                    return;
                };
                {
                    let mut graph = graph.borrow_mut();
                    let Some(device) = graph.devices.get_mut(&id) else {
                        return;
                    };
                    merge_route(device, update);
                }
                publish(&graph.borrow(), &states);
            }
        })
        .register();
    proxy.enum_params(0, Some(ParamType::Route), 0, u32::MAX);
    graph.borrow_mut().devices.insert(
        id,
        DeviceState {
            proxy,
            routes: HashMap::new(),
            _listener: listener,
        },
    );
}

fn add_defaults(
    registry: &Rc<Registry>,
    graph: &Rc<RefCell<Graph>>,
    states: &Rc<watch::Sender<AudioState>>,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    if props.get("metadata.name") != Some("default") {
        return;
    }
    let Ok(proxy) = registry.bind::<Metadata, _>(global) else {
        return;
    };
    let listener = proxy
        .add_listener_local()
        .property({
            let graph = graph.clone();
            let states = states.clone();
            move |_, key, _, value| {
                {
                    let mut graph = graph.borrow_mut();
                    match key {
                        Some("default.audio.sink") => {
                            graph.default_sink = value.and_then(default_node_name);
                        }
                        Some("default.audio.source") => {
                            graph.default_source = value.and_then(default_node_name);
                        }
                        Some(_) => return 0,
                        None => {
                            graph.default_sink = None;
                            graph.default_source = None;
                        }
                    }
                }
                publish(&graph.borrow(), &states);
                0
            }
        })
        .register();
    graph.borrow_mut().defaults.push(Defaults {
        _proxy: proxy,
        _listener: listener,
    });
}

fn node_route(props: &DictRef) -> Option<NodeRoute> {
    Some(NodeRoute {
        device: props.get("device.id")?.parse().ok()?,
        route_device: props.get("card.profile.device")?.parse().ok()?,
    })
}

fn merge_route(device: &mut DeviceState, update: RouteUpdate) {
    let route = device
        .routes
        .entry(update.device)
        .or_insert_with(|| RouteState {
            index: update.index,
            device: update.device,
            channel_volumes: Vec::new(),
            mute: false,
        });
    route.index = update.index;
    if let Some(volumes) = update.volumes {
        route.channel_volumes = volumes;
    }
    if let Some(mute) = update.mute {
        route.mute = mute;
    }
}

struct RouteTarget<'graph> {
    device: &'graph DeviceState,
    route: &'graph RouteState,
}

struct Levels {
    channel_volumes: Vec<f32>,
    mute: bool,
}

enum LevelChange {
    Volumes(Vec<f32>),
    Mute(bool),
}

fn target_node(graph: &Graph, target: Target) -> Option<&NodeState> {
    let name = match target {
        Target::DefaultSink => graph.default_sink.as_deref()?,
        Target::DefaultSource => graph.default_source.as_deref()?,
    };
    node_by_name(graph, name)
}

fn node_by_name<'graph>(graph: &'graph Graph, name: &str) -> Option<&'graph NodeState> {
    graph
        .nodes
        .iter()
        .filter(|(_, node)| node.name == name)
        .max_by_key(|(id, _)| **id)
        .map(|(_, node)| node)
}

fn route_of<'graph>(graph: &'graph Graph, node: &NodeState) -> Option<RouteTarget<'graph>> {
    let placement = node.route.as_ref()?;
    let device = graph.devices.get(&placement.device)?;
    let route = device.routes.get(&placement.route_device)?;
    Some(RouteTarget { device, route })
}

fn levels(graph: &Graph, node: &NodeState) -> Levels {
    match route_of(graph, node) {
        Some(target) => Levels {
            channel_volumes: target.route.channel_volumes.clone(),
            mute: target.route.mute,
        },
        None => Levels {
            channel_volumes: node.channel_volumes.clone(),
            mute: node.mute,
        },
    }
}

fn apply(graph: &Graph, target: Target, kind: CommandKind) -> Option<i64> {
    let node = target_node(graph, target)?;
    let levels = levels(graph, node);
    let current = volume_percent(&levels.channel_volumes);
    match kind {
        CommandKind::SetVolume(percent) => write_volume(graph, node, &levels, percent),
        CommandKind::AdjustVolume(delta) => {
            write_volume(graph, node, &levels, (current + delta).clamp(0, 100))
        }
        CommandKind::ToggleMute => {
            dispatch(graph, node, &LevelChange::Mute(!levels.mute))?;
            Some(current)
        }
    }
}

fn write_volume(graph: &Graph, node: &NodeState, levels: &Levels, percent: i64) -> Option<i64> {
    let channels = levels.channel_volumes.len();
    if channels == 0 {
        return None;
    }
    let volumes = vec![cubic(percent); channels];
    dispatch(graph, node, &LevelChange::Volumes(volumes))?;
    Some(percent)
}

fn dispatch(graph: &Graph, node: &NodeState, change: &LevelChange) -> Option<()> {
    if let Some(target) = route_of(graph, node) {
        let bytes = pod_bytes(&route_object(target.route, change))?;
        let pod = Pod::from_bytes(&bytes)?;
        target.device.proxy.set_param(ParamType::Route, 0, pod);
    } else {
        let bytes = pod_bytes(&props_object(vec![level_property(change)]))?;
        let pod = Pod::from_bytes(&bytes)?;
        node.proxy.set_param(ParamType::Props, 0, pod);
    }
    Some(())
}

fn publish(graph: &Graph, states: &watch::Sender<AudioState>) {
    states.send_replace(AudioState {
        sink: reading(graph, graph.default_sink.as_deref()),
        source: reading(graph, graph.default_source.as_deref()),
    });
}

fn reading(graph: &Graph, name: Option<&str>) -> Option<Reading> {
    let node = node_by_name(graph, name?)?;
    let levels = levels(graph, node);
    let percent = volume_percent(&levels.channel_volumes);
    Some(Reading {
        percent,
        muted: levels.mute || percent == 0,
        device: (!node.description.is_empty()).then(|| node.description.clone()),
    })
}

fn volume_percent(channel_volumes: &[f32]) -> i64 {
    let Some(&cubic) = channel_volumes.first() else {
        return 0;
    };
    #[allow(clippy::cast_possible_truncation)]
    let percent = (cubic.cbrt() * 100.0).round() as i64;
    percent.clamp(0, 100)
}

#[allow(clippy::cast_precision_loss)]
fn cubic(percent: i64) -> f32 {
    (percent as f32 / 100.0).powi(3)
}

fn default_node_name(value: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()?
        .get("name")?
        .as_str()
        .map(str::to_owned)
}

struct PropsUpdate {
    volumes: Option<Vec<f32>>,
    mute: Option<bool>,
}

struct RouteUpdate {
    index: u32,
    device: u32,
    volumes: Option<Vec<f32>>,
    mute: Option<bool>,
}

fn parse_props(pod: &Pod) -> Option<PropsUpdate> {
    let object = parse_object(pod, SPA_TYPE_OBJECT_Props)?;
    Some(PropsUpdate {
        volumes: volumes_of(&object),
        mute: mute_of(&object),
    })
}

fn parse_route(pod: &Pod) -> Option<RouteUpdate> {
    let object = parse_object(pod, SPA_TYPE_OBJECT_ParamRoute)?;
    let props = nested_props(&object);
    Some(RouteUpdate {
        index: int_of(&object, SPA_PARAM_ROUTE_index)?.try_into().ok()?,
        device: int_of(&object, SPA_PARAM_ROUTE_device)?.try_into().ok()?,
        volumes: props.as_ref().and_then(volumes_of),
        mute: props.as_ref().and_then(mute_of),
    })
}

fn parse_object(pod: &Pod, type_: u32) -> Option<Object> {
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?;
    match value {
        Value::Object(object) if object.type_ == type_ => Some(object),
        _ => None,
    }
}

fn nested_props(object: &Object) -> Option<Object> {
    object
        .properties
        .iter()
        .find_map(|property| match &property.value {
            Value::Object(nested) if property.key == SPA_PARAM_ROUTE_props => Some(nested.clone()),
            _ => None,
        })
}

fn volumes_of(object: &Object) -> Option<Vec<f32>> {
    object
        .properties
        .iter()
        .find_map(|property| match &property.value {
            Value::ValueArray(ValueArray::Float(values))
                if property.key == SPA_PROP_channelVolumes =>
            {
                Some(values.clone())
            }
            _ => None,
        })
}

fn mute_of(object: &Object) -> Option<bool> {
    object
        .properties
        .iter()
        .find_map(|property| match property.value {
            Value::Bool(value) if property.key == SPA_PROP_mute => Some(value),
            _ => None,
        })
}

fn int_of(object: &Object, key: u32) -> Option<i32> {
    object
        .properties
        .iter()
        .find_map(|property| match property.value {
            Value::Int(value) if property.key == key => Some(value),
            _ => None,
        })
}

fn level_property(change: &LevelChange) -> Property {
    match change {
        LevelChange::Volumes(volumes) => Property {
            key: SPA_PROP_channelVolumes,
            flags: PropertyFlags::empty(),
            value: Value::ValueArray(ValueArray::Float(volumes.clone())),
        },
        LevelChange::Mute(mute) => Property {
            key: SPA_PROP_mute,
            flags: PropertyFlags::empty(),
            value: Value::Bool(*mute),
        },
    }
}

fn props_object(properties: Vec<Property>) -> Value {
    Value::Object(Object {
        type_: SPA_TYPE_OBJECT_Props,
        id: SPA_PARAM_Props,
        properties,
    })
}

#[allow(clippy::cast_possible_wrap)]
fn route_object(route: &RouteState, change: &LevelChange) -> Value {
    Value::Object(Object {
        type_: SPA_TYPE_OBJECT_ParamRoute,
        id: SPA_PARAM_Route,
        properties: vec![
            Property {
                key: SPA_PARAM_ROUTE_index,
                flags: PropertyFlags::empty(),
                value: Value::Int(route.index as i32),
            },
            Property {
                key: SPA_PARAM_ROUTE_device,
                flags: PropertyFlags::empty(),
                value: Value::Int(route.device as i32),
            },
            Property {
                key: SPA_PARAM_ROUTE_props,
                flags: PropertyFlags::empty(),
                value: props_object(vec![level_property(change)]),
            },
            Property {
                key: SPA_PARAM_ROUTE_save,
                flags: PropertyFlags::empty(),
                value: Value::Bool(true),
            },
        ],
    })
}

fn pod_bytes(value: &Value) -> Option<Vec<u8>> {
    let (cursor, _) = PodSerializer::serialize(io::Cursor::new(Vec::new()), value).ok()?;
    Some(cursor.into_inner())
}
