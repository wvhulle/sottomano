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
    device::{Device, DeviceChangeMask, DeviceInfoRef, DeviceListener},
    main_loop::MainLoop,
    metadata::{Metadata, MetadataListener},
    node::{Node, NodeInfoRef, NodeListener},
    proxy::ProxyT,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub percent: i64,
    pub muted: bool,
    pub device: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
            if let Err(e) = thread::Builder::new()
                .name("pipewire-audio".to_owned())
                .spawn(move || run(&states, requests))
            {
                eprintln!("audio: pipewire thread spawn failed: {e}");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Output,
    Input,
}

#[derive(Debug, Default, PartialEq)]
struct Levels {
    channel_volumes: Vec<f32>,
    mute: bool,
}

impl Levels {
    fn percent(&self) -> i64 {
        let Some(&cubic) = self.channel_volumes.first() else {
            return 0;
        };
        #[allow(clippy::cast_possible_truncation)]
        let percent = (cubic.cbrt() * 100.0).round() as i64;
        percent.clamp(0, 100)
    }

    fn merge(&mut self, update: LevelsUpdate) {
        if let Some(volumes) = update.channel_volumes {
            self.channel_volumes = volumes;
        }
        if let Some(mute) = update.mute {
            self.mute = mute;
        }
    }
}

#[derive(Debug, Default, PartialEq)]
struct LevelsUpdate {
    channel_volumes: Option<Vec<f32>>,
    mute: Option<bool>,
}

#[derive(Debug, PartialEq, Eq)]
struct RoutePlacement {
    device: u32,
    route_device: u32,
}

#[derive(Debug)]
struct AudioNode {
    name: String,
    description: String,
    serial: u64,
    placement: Option<RoutePlacement>,
    levels: Levels,
}

#[derive(Debug)]
struct Route {
    index: u32,
    levels: Levels,
}

#[derive(Debug, Default)]
struct Graph {
    nodes: HashMap<u32, AudioNode>,
    routes: HashMap<u32, HashMap<u32, Route>>,
    default_sink: Option<String>,
    default_source: Option<String>,
}

#[derive(Debug)]
enum Event {
    NodeAdded {
        id: u32,
        node: AudioNode,
    },
    NodePlacement {
        id: u32,
        placement: RoutePlacement,
    },
    NodeLevels {
        id: u32,
        update: LevelsUpdate,
    },
    RouteLevels {
        device: u32,
        route: RouteLevels,
    },
    DefaultChanged {
        direction: Direction,
        name: Option<String>,
    },
    Removed {
        id: u32,
    },
}

#[derive(Debug, PartialEq)]
struct RouteLevels {
    index: u32,
    route_device: u32,
    update: LevelsUpdate,
}

fn reduce(graph: &mut Graph, event: Event) {
    match event {
        Event::NodeAdded { id, node } => {
            graph.nodes.insert(id, node);
        }
        Event::NodePlacement { id, placement } => {
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.placement = Some(placement);
            }
        }
        Event::NodeLevels { id, update } => {
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.levels.merge(update);
            }
        }
        Event::RouteLevels { device, route } => {
            let entry = graph
                .routes
                .entry(device)
                .or_default()
                .entry(route.route_device)
                .or_insert_with(|| Route {
                    index: route.index,
                    levels: Levels::default(),
                });
            entry.index = route.index;
            entry.levels.merge(route.update);
        }
        Event::DefaultChanged { direction, name } => match direction {
            Direction::Output => graph.default_sink = name,
            Direction::Input => graph.default_source = name,
        },
        Event::Removed { id } => {
            graph.nodes.remove(&id);
            graph.routes.remove(&id);
        }
    }
}

fn node_id_by_name(graph: &Graph, name: &str) -> Option<u32> {
    graph
        .nodes
        .iter()
        .filter(|(_, node)| node.name == name)
        .max_by_key(|(id, node)| (node.serial, **id))
        .map(|(id, _)| *id)
}

fn default_node_id(graph: &Graph, target: Target) -> Option<u32> {
    let name = match target {
        Target::DefaultSink => graph.default_sink.as_deref()?,
        Target::DefaultSource => graph.default_source.as_deref()?,
    };
    node_id_by_name(graph, name)
}

fn route_levels<'graph>(graph: &'graph Graph, node: &AudioNode) -> Option<&'graph Levels> {
    let placement = node.placement.as_ref()?;
    let route = graph.routes.get(&placement.device)?;
    Some(&route.get(&placement.route_device)?.levels)
}

fn effective_levels<'graph>(graph: &'graph Graph, node: &'graph AudioNode) -> &'graph Levels {
    route_levels(graph, node).unwrap_or(&node.levels)
}

fn reading(graph: &Graph, name: Option<&str>) -> Option<Reading> {
    let node = graph.nodes.get(&node_id_by_name(graph, name?)?)?;
    let levels = effective_levels(graph, node);
    let percent = levels.percent();
    Some(Reading {
        percent,
        muted: levels.mute || percent == 0,
        device: (!node.description.is_empty()).then(|| node.description.clone()),
    })
}

fn snapshot(graph: &Graph) -> AudioState {
    AudioState {
        sink: reading(graph, graph.default_sink.as_deref()),
        source: reading(graph, graph.default_source.as_deref()),
    }
}

enum LevelChange {
    Volumes(Vec<f32>),
    Mute(bool),
}

struct Resolved {
    change: LevelChange,
    percent: i64,
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
            change: LevelChange::Mute(!levels.mute),
            percent: current,
        }),
    }
}

#[allow(clippy::cast_precision_loss)]
fn cubic(percent: i64) -> f32 {
    (percent as f32 / 100.0).powi(3)
}

struct NodeProxy {
    proxy: Node,
    _listener: NodeListener,
}

struct DeviceProxy {
    proxy: Device,
    _listener: DeviceListener,
}

struct DefaultsProxy {
    _proxy: Metadata,
    _listener: MetadataListener,
}

#[derive(Default)]
struct Proxies {
    nodes: HashMap<u32, NodeProxy>,
    devices: HashMap<u32, DeviceProxy>,
    defaults: Vec<DefaultsProxy>,
}

#[derive(Default)]
struct Session {
    graph: Graph,
    proxies: Proxies,
}

enum Destination<'session> {
    Route {
        device: &'session Device,
        index: u32,
        route_device: u32,
    },
    NodeProps(&'session Node),
}

fn destination(session: &Session, id: u32) -> Option<Destination<'_>> {
    route_destination(session, id).or_else(|| {
        Some(Destination::NodeProps(
            &session.proxies.nodes.get(&id)?.proxy,
        ))
    })
}

fn route_destination(session: &Session, id: u32) -> Option<Destination<'_>> {
    let placement = session.graph.nodes.get(&id)?.placement.as_ref()?;
    let route = session
        .graph
        .routes
        .get(&placement.device)?
        .get(&placement.route_device)?;
    Some(Destination::Route {
        device: &session.proxies.devices.get(&placement.device)?.proxy,
        index: route.index,
        route_device: placement.route_device,
    })
}

fn apply(session: &Session, target: Target, kind: CommandKind) -> Option<i64> {
    let id = default_node_id(&session.graph, target)?;
    let node = session.graph.nodes.get(&id)?;
    let resolved = resolve(effective_levels(&session.graph, node), kind)?;
    write(session, id, &resolved.change)?;
    Some(resolved.percent)
}

fn write(session: &Session, id: u32, change: &LevelChange) -> Option<()> {
    match destination(session, id)? {
        Destination::Route {
            device,
            index,
            route_device,
        } => {
            let bytes = pod_bytes(&route_object(index, route_device, change))?;
            device.set_param(ParamType::Route, 0, Pod::from_bytes(&bytes)?);
        }
        Destination::NodeProps(node) => {
            let bytes = pod_bytes(&props_object(vec![level_property(change)]))?;
            node.set_param(ParamType::Props, 0, Pod::from_bytes(&bytes)?);
        }
    }
    Some(())
}

type Publisher = Rc<watch::Sender<AudioState>>;
type Shared = Rc<RefCell<Session>>;

fn dispatch(session: &Shared, states: &Publisher, event: Event) {
    reduce(&mut session.borrow_mut().graph, event);
    let next = snapshot(&session.borrow().graph);
    states.send_if_modified(|current| {
        if *current == next {
            return false;
        }
        report_unresolved(current, &next, &session.borrow().graph);
        *current = next;
        true
    });
}

fn report_unresolved(previous: &AudioState, next: &AudioState, graph: &Graph) {
    if previous.sink.is_some() && next.sink.is_none() {
        eprintln!(
            "audio: default sink {:?} no longer resolves to a node",
            graph.default_sink
        );
    }
    if previous.source.is_some() && next.source.is_none() {
        eprintln!(
            "audio: default source {:?} no longer resolves to a node",
            graph.default_source
        );
    }
}

fn run(states: &watch::Sender<AudioState>, commands: CommandReceiver<Command>) {
    pipewire::init();
    let states = Rc::new(states.clone());
    if let Err(e) = serve(&states, commands) {
        eprintln!("audio: pipewire connect failed: {e}");
    }
    states.send_replace(AudioState::default());
}

fn serve(states: &Publisher, commands: CommandReceiver<Command>) -> Result<(), pipewire::Error> {
    let main_loop = MainLoop::new(None)?;
    let context = Context::new(&main_loop)?;
    let core = context.connect(None)?;
    let registry = Rc::new(core.get_registry()?);
    let session = Shared::default();

    let _attached = commands.attach(main_loop.loop_(), {
        let session = session.clone();
        move |command: Command| {
            let outcome = (Instant::now() <= command.expires_at)
                .then(|| apply(&session.borrow(), command.target, command.kind))
                .flatten();
            let _ = command.reply.send(outcome);
        }
    });

    let _core_listener = core
        .add_listener_local()
        .error(|id, _, res, message| {
            eprintln!("audio: pipewire object {id} reported an error ({res}): {message}");
        })
        .register();

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let registry = registry.clone();
            let session = session.clone();
            let states = states.clone();
            move |global| track(&registry, &session, &states, global)
        })
        .global_remove({
            let session = session.clone();
            let states = states.clone();
            move |id| {
                {
                    let mut owned = session.borrow_mut();
                    owned.proxies.nodes.remove(&id);
                    owned.proxies.devices.remove(&id);
                }
                dispatch(&session, &states, Event::Removed { id });
            }
        })
        .register();

    main_loop.run();
    Ok(())
}

fn track(
    registry: &Registry,
    session: &Shared,
    states: &Publisher,
    global: &GlobalObject<&DictRef>,
) {
    let Some(props) = global.props else {
        return;
    };
    match global.type_ {
        ObjectType::Node => track_node(registry, session, states, global, props),
        ObjectType::Device => track_device(registry, session, states, global, props),
        ObjectType::Metadata => track_defaults(registry, session, states, global, props),
        _ => {}
    }
}

fn bind<P: ProxyT>(registry: &Registry, global: &GlobalObject<&DictRef>, label: &str) -> Option<P> {
    registry
        .bind::<P, _>(global)
        .inspect_err(|e| eprintln!("audio: bind {label} {} failed: {e}", global.id))
        .ok()
}

fn track_node(
    registry: &Registry,
    session: &Shared,
    states: &Publisher,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    if node_direction(props).is_none() {
        return;
    }
    let Some(proxy) = bind::<Node>(registry, global, "node") else {
        return;
    };
    proxy.subscribe_params(&[ParamType::Props]);
    let id = global.id;
    let listener = proxy
        .add_listener_local()
        .info(on_node_info(id, session, states))
        .param(on_node_param(id, session, states))
        .register();
    dispatch(
        session,
        states,
        Event::NodeAdded {
            id,
            node: node_from_props(props),
        },
    );
    session.borrow_mut().proxies.nodes.insert(
        id,
        NodeProxy {
            proxy,
            _listener: listener,
        },
    );
}

fn track_device(
    registry: &Registry,
    session: &Shared,
    states: &Publisher,
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
        .info(on_device_info(id, session))
        .param(on_device_param(id, session, states))
        .register();
    proxy.enum_params(0, Some(ParamType::Route), 0, u32::MAX);
    session.borrow_mut().proxies.devices.insert(
        id,
        DeviceProxy {
            proxy,
            _listener: listener,
        },
    );
}

fn track_defaults(
    registry: &Registry,
    session: &Shared,
    states: &Publisher,
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
        .property(on_default_property(session, states))
        .register();
    session.borrow_mut().proxies.defaults.push(DefaultsProxy {
        _proxy: proxy,
        _listener: listener,
    });
}

fn on_node_info(id: u32, session: &Shared, states: &Publisher) -> impl Fn(&NodeInfoRef) + 'static {
    let session = session.clone();
    let states = states.clone();
    move |info| {
        if let Some(placement) = info.props().and_then(route_placement) {
            dispatch(&session, &states, Event::NodePlacement { id, placement });
        }
    }
}

fn on_node_param(
    id: u32,
    session: &Shared,
    states: &Publisher,
) -> impl Fn(i32, ParamType, u32, u32, Option<&Pod>) + 'static {
    let session = session.clone();
    let states = states.clone();
    move |_, _, _, _, param| {
        if let Some(update) = param.and_then(parse_props) {
            dispatch(&session, &states, Event::NodeLevels { id, update });
        }
    }
}

fn on_device_info(id: u32, session: &Shared) -> impl Fn(&DeviceInfoRef) + 'static {
    let session = session.clone();
    move |info| {
        if info.change_mask().contains(DeviceChangeMask::PARAMS)
            && let Some(device) = session.borrow().proxies.devices.get(&id)
        {
            device
                .proxy
                .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
        }
    }
}

fn on_device_param(
    id: u32,
    session: &Shared,
    states: &Publisher,
) -> impl Fn(i32, ParamType, u32, u32, Option<&Pod>) + 'static {
    let session = session.clone();
    let states = states.clone();
    move |_, param_type, _, _, param| {
        if param_type != ParamType::Route {
            return;
        }
        if let Some(route) = param.and_then(parse_route) {
            dispatch(&session, &states, Event::RouteLevels { device: id, route });
        }
    }
}

fn on_default_property(
    session: &Shared,
    states: &Publisher,
) -> impl Fn(u32, Option<&str>, Option<&str>, Option<&str>) -> i32 + 'static {
    let session = session.clone();
    let states = states.clone();
    move |_, key, _, value| {
        if let Some(event) = default_event(key, value) {
            dispatch(&session, &states, event);
        }
        0
    }
}

fn default_event(key: Option<&str>, value: Option<&str>) -> Option<Event> {
    let changed = |direction| Event::DefaultChanged {
        direction,
        name: value.and_then(default_node_name),
    };
    match key? {
        "default.audio.sink" => Some(changed(Direction::Output)),
        "default.audio.source" => Some(changed(Direction::Input)),
        _ => None,
    }
}

fn node_direction(props: &DictRef) -> Option<Direction> {
    match props.get("media.class")? {
        "Audio/Sink" => Some(Direction::Output),
        "Audio/Source" => Some(Direction::Input),
        _ => None,
    }
}

fn node_from_props(props: &DictRef) -> AudioNode {
    AudioNode {
        name: props.get("node.name").unwrap_or_default().to_owned(),
        description: props
            .get("node.nick")
            .or_else(|| props.get("node.description"))
            .unwrap_or_default()
            .to_owned(),
        serial: props
            .get("object.serial")
            .and_then(|serial| serial.parse().ok())
            .unwrap_or_default(),
        placement: None,
        levels: Levels::default(),
    }
}

fn route_placement(props: &DictRef) -> Option<RoutePlacement> {
    Some(RoutePlacement {
        device: props.get("device.id")?.parse().ok()?,
        route_device: props.get("card.profile.device")?.parse().ok()?,
    })
}

fn default_node_name(value: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()?
        .get("name")?
        .as_str()
        .map(str::to_owned)
}

fn parse_props(pod: &Pod) -> Option<LevelsUpdate> {
    parse_object(pod, SPA_TYPE_OBJECT_Props)
        .as_ref()
        .map(levels_update)
}

fn parse_route(pod: &Pod) -> Option<RouteLevels> {
    let object = parse_object(pod, SPA_TYPE_OBJECT_ParamRoute)?;
    Some(RouteLevels {
        index: int_of(&object, SPA_PARAM_ROUTE_index)?.try_into().ok()?,
        route_device: int_of(&object, SPA_PARAM_ROUTE_device)?.try_into().ok()?,
        update: nested_props(&object)
            .as_ref()
            .map(levels_update)
            .unwrap_or_default(),
    })
}

fn parse_object(pod: &Pod, type_: u32) -> Option<Object> {
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?;
    match value {
        Value::Object(object) if object.type_ == type_ => Some(object),
        _ => None,
    }
}

fn levels_update(object: &Object) -> LevelsUpdate {
    LevelsUpdate {
        channel_volumes: volumes_of(object),
        mute: mute_of(object),
    }
}

fn nested_props(object: &Object) -> Option<Object> {
    property_value(object, SPA_PARAM_ROUTE_props).and_then(|value| match value {
        Value::Object(nested) => Some(nested.clone()),
        _ => None,
    })
}

fn volumes_of(object: &Object) -> Option<Vec<f32>> {
    match property_value(object, SPA_PROP_channelVolumes)? {
        Value::ValueArray(ValueArray::Float(values)) => Some(values.clone()),
        _ => None,
    }
}

fn mute_of(object: &Object) -> Option<bool> {
    match property_value(object, SPA_PROP_mute)? {
        Value::Bool(value) => Some(*value),
        _ => None,
    }
}

fn int_of(object: &Object, key: u32) -> Option<i32> {
    match property_value(object, key)? {
        Value::Int(value) => Some(*value),
        _ => None,
    }
}

fn property_value(object: &Object, key: u32) -> Option<&Value> {
    object
        .properties
        .iter()
        .find(|property| property.key == key)
        .map(|property| &property.value)
}

fn level_property(change: &LevelChange) -> Property {
    let (key, value) = match change {
        LevelChange::Volumes(volumes) => (
            SPA_PROP_channelVolumes,
            Value::ValueArray(ValueArray::Float(volumes.clone())),
        ),
        LevelChange::Mute(mute) => (SPA_PROP_mute, Value::Bool(*mute)),
    };
    Property {
        key,
        flags: PropertyFlags::empty(),
        value,
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
fn route_object(index: u32, route_device: u32, change: &LevelChange) -> Value {
    let field = |key, value| Property {
        key,
        flags: PropertyFlags::empty(),
        value,
    };
    Value::Object(Object {
        type_: SPA_TYPE_OBJECT_ParamRoute,
        id: SPA_PARAM_Route,
        properties: vec![
            field(SPA_PARAM_ROUTE_index, Value::Int(index as i32)),
            field(SPA_PARAM_ROUTE_device, Value::Int(route_device as i32)),
            field(
                SPA_PARAM_ROUTE_props,
                props_object(vec![level_property(change)]),
            ),
            field(SPA_PARAM_ROUTE_save, Value::Bool(true)),
        ],
    })
}

fn pod_bytes(value: &Value) -> Option<Vec<u8>> {
    let (cursor, _) = PodSerializer::serialize(io::Cursor::new(Vec::new()), value).ok()?;
    Some(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, volumes: &[f32]) -> AudioNode {
        serialised(name, volumes, 0)
    }

    fn serialised(name: &str, volumes: &[f32], serial: u64) -> AudioNode {
        AudioNode {
            name: name.to_owned(),
            description: name.to_owned(),
            serial,
            placement: None,
            levels: Levels {
                channel_volumes: volumes.to_vec(),
                mute: false,
            },
        }
    }

    fn graph_with(events: Vec<Event>) -> Graph {
        let mut graph = Graph::default();
        for event in events {
            reduce(&mut graph, event);
        }
        graph
    }

    #[test]
    fn percent_round_trips_through_cubic_volume() {
        for percent in 0..=100 {
            let levels = Levels {
                channel_volumes: vec![cubic(percent)],
                mute: false,
            };
            assert_eq!(levels.percent(), percent);
        }
    }

    #[test]
    fn reading_prefers_route_over_node_props() {
        let graph = graph_with(vec![
            Event::NodeAdded {
                id: 56,
                node: node("speaker", &[cubic(54)]),
            },
            Event::NodePlacement {
                id: 56,
                placement: RoutePlacement {
                    device: 46,
                    route_device: 3,
                },
            },
            Event::RouteLevels {
                device: 46,
                route: RouteLevels {
                    index: 3,
                    route_device: 3,
                    update: LevelsUpdate {
                        channel_volumes: Some(vec![cubic(29)]),
                        mute: None,
                    },
                },
            },
            Event::DefaultChanged {
                direction: Direction::Output,
                name: Some("speaker".to_owned()),
            },
        ]);
        assert_eq!(snapshot(&graph).sink.unwrap().percent, 29);
    }

    #[test]
    fn reading_falls_back_to_node_props_without_route() {
        let graph = graph_with(vec![
            Event::NodeAdded {
                id: 7,
                node: node("null-sink", &[cubic(42)]),
            },
            Event::DefaultChanged {
                direction: Direction::Output,
                name: Some("null-sink".to_owned()),
            },
        ]);
        assert_eq!(snapshot(&graph).sink.unwrap().percent, 42);
    }

    #[test]
    fn highest_serial_wins_when_names_collide_on_a_recycled_id() {
        let graph = graph_with(vec![
            Event::NodeAdded {
                id: 91,
                node: serialised("bluez", &[cubic(10)], 1593),
            },
            Event::NodeAdded {
                id: 90,
                node: serialised("bluez", &[cubic(70)], 29349),
            },
            Event::DefaultChanged {
                direction: Direction::Output,
                name: Some("bluez".to_owned()),
            },
        ]);
        assert_eq!(snapshot(&graph).sink.unwrap().percent, 70);
    }

    #[test]
    fn a_headset_connect_then_disconnect_round_trips() {
        let built_in = vec![
            Event::NodeAdded {
                id: 58,
                node: serialised("mic1", &[cubic(60)], 58),
            },
            Event::DefaultChanged {
                direction: Direction::Input,
                name: Some("mic1".to_owned()),
            },
        ];
        let mut graph = graph_with(built_in);
        assert_eq!(snapshot(&graph).source.unwrap().percent, 60);

        for event in [
            Event::NodeAdded {
                id: 97,
                node: serialised("bluez_in", &[], 1593),
            },
            Event::NodePlacement {
                id: 97,
                placement: RoutePlacement {
                    device: 93,
                    route_device: 0,
                },
            },
            Event::RouteLevels {
                device: 93,
                route: RouteLevels {
                    index: 0,
                    route_device: 0,
                    update: LevelsUpdate {
                        channel_volumes: Some(vec![cubic(35)]),
                        mute: Some(false),
                    },
                },
            },
            Event::DefaultChanged {
                direction: Direction::Input,
                name: Some("bluez_in".to_owned()),
            },
        ] {
            reduce(&mut graph, event);
        }
        assert_eq!(snapshot(&graph).source.unwrap().percent, 35);

        reduce(&mut graph, Event::Removed { id: 97 });
        reduce(&mut graph, Event::Removed { id: 93 });
        assert!(snapshot(&graph).source.is_none());

        reduce(
            &mut graph,
            Event::DefaultChanged {
                direction: Direction::Input,
                name: Some("mic1".to_owned()),
            },
        );
        assert_eq!(snapshot(&graph).source.unwrap().percent, 60);
    }

    #[test]
    fn a_bluez_profile_switch_recovers_the_reading() {
        let mut graph = graph_with(vec![
            Event::NodeAdded {
                id: 97,
                node: serialised("bluez_in", &[cubic(35)], 1593),
            },
            Event::DefaultChanged {
                direction: Direction::Input,
                name: Some("bluez_in".to_owned()),
            },
        ]);
        assert_eq!(snapshot(&graph).source.unwrap().percent, 35);

        reduce(&mut graph, Event::Removed { id: 97 });
        assert!(snapshot(&graph).source.is_none());

        reduce(
            &mut graph,
            Event::NodeAdded {
                id: 101,
                node: serialised("bluez_in", &[cubic(35)], 29349),
            },
        );
        assert_eq!(snapshot(&graph).source.unwrap().percent, 35);
    }

    #[test]
    fn removing_a_device_drops_its_routes() {
        let mut graph = graph_with(vec![
            Event::NodeAdded {
                id: 56,
                node: node("speaker", &[cubic(54)]),
            },
            Event::NodePlacement {
                id: 56,
                placement: RoutePlacement {
                    device: 46,
                    route_device: 3,
                },
            },
            Event::RouteLevels {
                device: 46,
                route: RouteLevels {
                    index: 3,
                    route_device: 3,
                    update: LevelsUpdate {
                        channel_volumes: Some(vec![cubic(29)]),
                        mute: None,
                    },
                },
            },
            Event::DefaultChanged {
                direction: Direction::Output,
                name: Some("speaker".to_owned()),
            },
        ]);
        reduce(&mut graph, Event::Removed { id: 46 });
        assert_eq!(snapshot(&graph).sink.unwrap().percent, 54);
    }

    #[test]
    fn a_metadata_clear_never_forgets_the_default() {
        let mut graph = graph_with(vec![
            Event::NodeAdded {
                id: 56,
                node: node("speaker", &[cubic(54)]),
            },
            Event::DefaultChanged {
                direction: Direction::Output,
                name: Some("speaker".to_owned()),
            },
        ]);
        assert!(default_event(None, None).is_none());
        assert_eq!(snapshot(&graph).sink.unwrap().percent, 54);
        reduce(&mut graph, Event::Removed { id: 56 });
        assert!(snapshot(&graph).sink.is_none());
    }

    #[test]
    fn partial_levels_update_keeps_untouched_fields() {
        let mut levels = Levels {
            channel_volumes: vec![cubic(30), cubic(30)],
            mute: false,
        };
        levels.merge(LevelsUpdate {
            channel_volumes: None,
            mute: Some(true),
        });
        assert_eq!(levels.percent(), 30);
        assert!(levels.mute);
    }

    #[test]
    fn adjust_resolves_against_current_level() {
        let levels = Levels {
            channel_volumes: vec![cubic(33), cubic(33)],
            mute: false,
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
            mute: false,
        };
        assert_eq!(
            resolve(&levels, CommandKind::AdjustVolume(5))
                .unwrap()
                .percent,
            100
        );
    }

    #[test]
    fn resolve_refuses_before_channel_count_is_known() {
        let levels = Levels::default();
        assert!(resolve(&levels, CommandKind::SetVolume(50)).is_none());
    }

    #[test]
    fn route_pod_round_trips_through_the_parser() {
        let change = LevelChange::Volumes(vec![cubic(38), cubic(38)]);
        let bytes = pod_bytes(&route_object(3, 3, &change)).unwrap();
        let parsed = parse_route(Pod::from_bytes(&bytes).unwrap()).unwrap();
        assert_eq!(
            parsed,
            RouteLevels {
                index: 3,
                route_device: 3,
                update: LevelsUpdate {
                    channel_volumes: Some(vec![cubic(38), cubic(38)]),
                    mute: None,
                },
            }
        );
    }

    #[test]
    fn props_pod_round_trips_mute() {
        let bytes = pod_bytes(&props_object(vec![level_property(&LevelChange::Mute(
            true,
        ))]))
        .unwrap();
        let parsed = parse_props(Pod::from_bytes(&bytes).unwrap()).unwrap();
        assert_eq!(
            parsed,
            LevelsUpdate {
                channel_volumes: None,
                mute: Some(true),
            }
        );
    }

    #[test]
    fn metadata_keys_map_to_events() {
        let sink = default_event(Some("default.audio.sink"), Some(r#"{"name":"speaker"}"#));
        assert!(matches!(
            sink,
            Some(Event::DefaultChanged {
                direction: Direction::Output,
                name: Some(ref name),
            }) if name == "speaker"
        ));
        assert!(default_event(None, None).is_none());
        assert!(default_event(Some("default.video.source"), None).is_none());
    }
}
