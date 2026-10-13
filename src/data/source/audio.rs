use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    convert::Infallible,
    env, fmt, io,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        mpsc::{SyncSender, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

use iced::{
    Subscription,
    futures::{Stream, stream::BoxStream},
};
use pipewire::{
    channel::{Receiver as CommandReceiver, Sender as CommandSender, channel as command_channel},
    context::Context,
    device::{Device, DeviceChangeMask, DeviceInfoRef, DeviceListener},
    main_loop::MainLoop,
    metadata::{Metadata, MetadataListener},
    node::{Node, NodeInfoRef, NodeListener, NodeState},
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
use tokio::sync::{oneshot, watch};
use tokio_stream::{self as stream, StreamExt as _, wrappers::WatchStream};

use crate::{
    data::source::{
        Delivery, LiveSource, Source, file_presence::path_present, present_or_absent, settled,
        while_present,
    },
    message_subscriptions::restarting,
};

mod meter;

pub use meter::Peaks;

const REPLY_TIMEOUT: Duration = Duration::from_millis(750);
const DEFAULT_REMOTE: &str = "pipewire-0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub percent: i64,
    pub muted: bool,
    pub device: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAudio {
    pub playing: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputKind {
    #[default]
    Speaker,
    Headphones,
    Headset,
    Display,
}

impl OutputKind {
    fn of_node(form_factor: Option<&str>, node_name: &str) -> Self {
        form_factor
            .and_then(Self::of_form_factor)
            .unwrap_or_else(|| Self::of_node_name(node_name))
    }

    fn of_form_factor(form_factor: &str) -> Option<Self> {
        match form_factor {
            "headphone" => Some(Self::Headphones),
            "headset" | "hands-free" | "handset" => Some(Self::Headset),
            "tv" | "monitor" => Some(Self::Display),
            "speaker" | "internal" | "computer" | "portable" | "hifi" | "car" => {
                Some(Self::Speaker)
            }
            _ => None,
        }
    }

    fn of_node_name(node_name: &str) -> Self {
        let name = node_name.to_ascii_lowercase();
        if name.contains("headphone") {
            Self::Headphones
        } else if name.contains("headset") {
            Self::Headset
        } else if name.contains("hdmi") || name.contains("displayport") {
            Self::Display
        } else {
            Self::Speaker
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioState {
    pub sink: Option<Reading>,
    pub source: Option<Reading>,
    pub output_kind: OutputKind,
    pub windows: Arc<HashMap<i32, WindowAudio>>,
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    Connected(Commands),
    State(AudioState),
    Peaks(Peaks),
}

#[derive(Debug, Clone, Copy)]
pub enum Target {
    DefaultSink,
    DefaultSource,
    AppStreams(i32),
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

    pub fn set_mute(&self, target: Target, muted: bool) -> Option<i64> {
        self.request(target, CommandKind::SetMute(muted))
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

pub fn subscription() -> Subscription<Option<AudioEvent>> {
    Subscription::run(|| while_present(socket_present(), || Box::pin(restarting("audio", connect))))
}

fn socket_present() -> BoxStream<'static, bool> {
    match pipewire_socket() {
        Ok(socket) => Box::pin(path_present(socket)),
        Err(e) => {
            tracing::warn!("audio: {e}");
            Box::pin(settled(false))
        }
    }
}

impl Source for AudioState {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "audio";
}

impl LiveSource for AudioState {
    fn events(_key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(socket_present(), || {
            Box::pin(
                restarting("audio", connect).filter_map(|event| match event {
                    AudioEvent::State(state) => Some(Delivery::Reading(Ok(state))),
                    AudioEvent::Connected(_) | AudioEvent::Peaks(_) => None,
                }),
            )
        })
    }
}

async fn connect() -> io::Result<impl Stream<Item = AudioEvent>> {
    let (states, updates) = watch::channel(AudioState::default());
    let (peaks, peak_updates) = watch::channel(Peaks::default());
    let (commands, requests) = command_channel::<Command>();
    let (connected, pipewire_connected) = oneshot::channel();
    thread::Builder::new()
        .name("pipewire-audio".to_owned())
        .spawn(move || run(&states, &peaks, requests, connected))?;
    pipewire_connected
        .await
        .map_err(|_| io::Error::other("pipewire connect failed"))?;
    let changes = WatchStream::from_changes(updates)
        .map(AudioEvent::State)
        .merge(WatchStream::from_changes(peak_updates).map(AudioEvent::Peaks));
    Ok(stream::iter([AudioEvent::Connected(Commands { sender: commands })]).chain(changes))
}

fn pipewire_socket() -> io::Result<PathBuf> {
    let Some(directory) =
        env::var_os("PIPEWIRE_RUNTIME_DIR").or_else(|| env::var_os("XDG_RUNTIME_DIR"))
    else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "neither PIPEWIRE_RUNTIME_DIR nor XDG_RUNTIME_DIR is set",
        ));
    };
    let remote = env::var_os("PIPEWIRE_REMOTE").unwrap_or_else(|| DEFAULT_REMOTE.into());
    Ok(PathBuf::from(directory).join(remote))
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
    SetMute(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Direction {
    Output,
    Input,
}

impl Direction {
    fn target(self) -> Target {
        match self {
            Direction::Output => Target::DefaultSink,
            Direction::Input => Target::DefaultSource,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Direction::Output => "sink",
            Direction::Input => "source",
        }
    }
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
    direction: Direction,
    name: String,
    description: Option<String>,
    output_kind: OutputKind,
    serial: u64,
    placement: Option<RoutePlacement>,
    levels: Levels,
}

#[derive(Debug)]
struct Route {
    index: u32,
    levels: Levels,
}

#[derive(Debug)]
struct AppStream {
    pid: Option<i32>,
    running: bool,
    levels: Levels,
}

#[derive(Debug, Default)]
struct Graph {
    nodes: HashMap<u32, AudioNode>,
    streams: HashMap<u32, AppStream>,
    routes: HashMap<u32, HashMap<u32, Route>>,
    default_sink: Option<String>,
    default_source: Option<String>,
}

impl Graph {
    fn default_name(&self, direction: Direction) -> Option<&str> {
        match direction {
            Direction::Output => self.default_sink.as_deref(),
            Direction::Input => self.default_source.as_deref(),
        }
    }

    fn node_count(&self, direction: Direction) -> usize {
        self.nodes
            .values()
            .filter(|node| node.direction == direction)
            .count()
    }
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
    Levels {
        id: u32,
        update: LevelsUpdate,
    },
    StreamAdded {
        id: u32,
        stream: AppStream,
    },
    StreamInfo {
        id: u32,
        pid: Option<i32>,
        running: bool,
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
        Event::Levels { id, update } => {
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.levels.merge(update);
            } else if let Some(stream) = graph.streams.get_mut(&id) {
                stream.levels.merge(update);
            }
        }
        Event::StreamAdded { id, stream } => {
            graph.streams.insert(id, stream);
        }
        Event::StreamInfo { id, pid, running } => {
            if let Some(stream) = graph.streams.get_mut(&id) {
                if pid.is_some() {
                    stream.pid = pid;
                }
                stream.running = running;
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
            graph.streams.remove(&id);
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
        Target::AppStreams(_) => return None,
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
        device: node.description.clone(),
    })
}

fn default_output_kind(graph: &Graph) -> OutputKind {
    graph
        .default_sink
        .as_deref()
        .and_then(|name| graph.nodes.get(&node_id_by_name(graph, name)?))
        .map(|node| node.output_kind)
        .unwrap_or_default()
}

fn snapshot(graph: &Graph) -> AudioState {
    AudioState {
        sink: reading(graph, graph.default_sink.as_deref()),
        source: reading(graph, graph.default_source.as_deref()),
        output_kind: default_output_kind(graph),
        windows: Arc::new(window_audio(graph)),
    }
}

fn window_audio(graph: &Graph) -> HashMap<i32, WindowAudio> {
    let mut by_pid: HashMap<i32, WindowAudio> = HashMap::new();
    for stream in graph.streams.values() {
        let Some(pid) = stream.pid else { continue };
        let entry = by_pid.entry(pid).or_insert(WindowAudio {
            playing: false,
            muted: true,
        });
        entry.playing |= stream.running;
        entry.muted &= stream.levels.mute;
    }
    by_pid
}

fn stream_ids(graph: &Graph, pid: i32) -> Vec<u32> {
    graph
        .streams
        .iter()
        .filter(|(_, stream)| stream.pid == Some(pid))
        .map(|(id, _)| *id)
        .collect()
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
        CommandKind::SetMute(muted) => Some(Resolved {
            change: LevelChange::Mute(muted),
            percent: current,
        }),
    }
}

#[allow(clippy::cast_precision_loss)]
fn cubic(percent: i64) -> f32 {
    (percent as f32 / 100.0).powi(3)
}

struct Bound<P, L> {
    proxy: P,
    _listener: L,
}

#[derive(Default)]
struct Proxies {
    nodes: HashMap<u32, Bound<Node, NodeListener>>,
    devices: HashMap<u32, Bound<Device, DeviceListener>>,
    defaults: Vec<Bound<Metadata, MetadataListener>>,
}

#[derive(Default)]
struct Session {
    graph: Graph,
    proxies: Proxies,
    reported_missing: HashSet<Direction>,
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
    if let Target::AppStreams(pid) = target {
        return apply_to_streams(session, pid, kind);
    }
    let id = default_node_id(&session.graph, target)?;
    let node = session.graph.nodes.get(&id)?;
    let resolved = resolve(effective_levels(&session.graph, node), kind)?;
    write(session, id, &resolved.change)?;
    Some(resolved.percent)
}

fn apply_to_streams(session: &Session, pid: i32, kind: CommandKind) -> Option<i64> {
    let ids = stream_ids(&session.graph, pid);
    let muted = !ids.is_empty() && ids.iter().all(|id| session.graph.streams[id].levels.mute);
    ids.iter()
        .filter_map(|id| {
            let levels = &session.graph.streams.get(id)?.levels;
            let resolved = match kind {
                CommandKind::ToggleMute => Resolved {
                    change: LevelChange::Mute(!muted),
                    percent: levels.percent(),
                },
                _ => resolve(levels, kind)?,
            };
            write(session, *id, &resolved.change)?;
            Some(resolved.percent)
        })
        .reduce(|_, percent| percent)
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
    report_missing_defaults(&mut session.borrow_mut());
}

fn report_missing_defaults(session: &mut Session) {
    for direction in [Direction::Output, Direction::Input] {
        let graph = &session.graph;
        let present = graph.node_count(direction);
        if default_node_id(graph, direction.target()).is_some() || present == 0 {
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

fn report_unresolved(previous: &AudioState, next: &AudioState, graph: &Graph) {
    if previous.sink.is_some() && next.sink.is_none() {
        tracing::warn!(
            "audio: default sink {:?} no longer resolves to a node",
            graph.default_sink
        );
    }
    if previous.source.is_some() && next.source.is_none() {
        tracing::warn!(
            "audio: default source {:?} no longer resolves to a node",
            graph.default_source
        );
    }
}

fn run(
    states: &watch::Sender<AudioState>,
    peaks: &watch::Sender<Peaks>,
    commands: CommandReceiver<Command>,
    connected: oneshot::Sender<()>,
) {
    pipewire::init();
    let states = Rc::new(states.clone());
    if let Err(e) = serve(&states, peaks, commands, connected) {
        tracing::error!("audio: pipewire connect failed: {e}");
    }
    states.send_replace(AudioState::default());
    peaks.send_replace(Peaks::default());
}

fn serve(
    states: &Publisher,
    peaks: &watch::Sender<Peaks>,
    commands: CommandReceiver<Command>,
    connected: oneshot::Sender<()>,
) -> Result<(), pipewire::Error> {
    let main_loop = MainLoop::new(None)?;
    let context = Context::new(&main_loop)?;
    let core = context.connect(None)?;
    let _ = connected.send(());
    let registry = Rc::new(core.get_registry()?);
    let _meters = meter::attach(&core, peaks);
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
            tracing::warn!("audio: pipewire object {id} reported an error ({res}): {message}");
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
        .inspect_err(|e| tracing::warn!("audio: bind {label} {} failed: {e}", global.id))
        .ok()
}

fn track_node(
    registry: &Registry,
    session: &Shared,
    states: &Publisher,
    global: &GlobalObject<&DictRef>,
    props: &DictRef,
) {
    let id = global.id;
    if let Some(direction) = node_direction(props) {
        if let Some(node) = node_from_props(props, direction) {
            track_levelled_node(
                registry,
                session,
                states,
                global,
                "node",
                on_node_info(id, session, states),
                Event::NodeAdded { id, node },
            );
        }
    } else if is_output_stream(props) {
        track_levelled_node(
            registry,
            session,
            states,
            global,
            "stream",
            on_stream_info(id, session, states),
            Event::StreamAdded {
                id,
                stream: stream_from_props(props),
            },
        );
    }
}

fn track_levelled_node(
    registry: &Registry,
    session: &Shared,
    states: &Publisher,
    global: &GlobalObject<&DictRef>,
    label: &str,
    on_info: impl Fn(&NodeInfoRef) + 'static,
    added: Event,
) {
    let Some(proxy) = bind::<Node>(registry, global, label) else {
        return;
    };
    proxy.subscribe_params(&[ParamType::Props]);
    let id = global.id;
    let listener = proxy
        .add_listener_local()
        .info(on_info)
        .param(on_levels_param(id, session, states))
        .register();
    dispatch(session, states, added);
    session.borrow_mut().proxies.nodes.insert(
        id,
        Bound {
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
        Bound {
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
    session.borrow_mut().proxies.defaults.push(Bound {
        proxy,
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

fn on_levels_param(
    id: u32,
    session: &Shared,
    states: &Publisher,
) -> impl Fn(i32, ParamType, u32, u32, Option<&Pod>) + 'static {
    let session = session.clone();
    let states = states.clone();
    move |_, _, _, _, param| {
        if let Some(update) = param.and_then(parse_props) {
            dispatch(&session, &states, Event::Levels { id, update });
        }
    }
}

fn on_stream_info(
    id: u32,
    session: &Shared,
    states: &Publisher,
) -> impl Fn(&NodeInfoRef) + 'static {
    let session = session.clone();
    let states = states.clone();
    move |info| {
        let pid = info.props().and_then(stream_pid);
        let running = matches!(info.state(), NodeState::Running);
        dispatch(&session, &states, Event::StreamInfo { id, pid, running });
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

fn is_output_stream(props: &DictRef) -> bool {
    props.get("media.class") == Some("Stream/Output/Audio")
}

fn stream_pid(props: &DictRef) -> Option<i32> {
    props.get("application.process.id")?.parse().ok()
}

fn stream_from_props(props: &DictRef) -> AppStream {
    AppStream {
        pid: stream_pid(props),
        running: false,
        levels: Levels::default(),
    }
}

fn node_from_props(props: &DictRef, direction: Direction) -> Option<AudioNode> {
    let name = props.get("node.name")?;
    Some(AudioNode {
        direction,
        name: name.to_owned(),
        description: props
            .get("node.nick")
            .or_else(|| props.get("node.description"))
            .filter(|description| !description.is_empty())
            .map(str::to_owned),
        output_kind: OutputKind::of_node(props.get("device.form-factor"), name),
        serial: props
            .get("object.serial")
            .and_then(|serial| serial.parse().ok())
            .unwrap_or_default(),
        placement: None,
        levels: Levels::default(),
    })
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

    fn node(direction: Direction, name: &str, volumes: &[f32]) -> AudioNode {
        AudioNode {
            direction,
            name: name.to_owned(),
            description: Some(name.to_owned()),
            output_kind: OutputKind::default(),
            serial: 0,
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
    fn reading_prefers_route_over_node_props() {
        let graph = graph_with(vec![
            Event::NodeAdded {
                id: 56,
                node: node(Direction::Output, "speaker", &[cubic(54)]),
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
                node: node(Direction::Output, "null-sink", &[cubic(42)]),
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
                node: AudioNode {
                    serial: 1593,
                    ..node(Direction::Output, "bluez", &[cubic(10)])
                },
            },
            Event::NodeAdded {
                id: 90,
                node: AudioNode {
                    serial: 29349,
                    ..node(Direction::Output, "bluez", &[cubic(70)])
                },
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
                node: AudioNode {
                    serial: 58,
                    ..node(Direction::Input, "mic1", &[cubic(60)])
                },
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
                node: AudioNode {
                    serial: 1593,
                    ..node(Direction::Input, "bluez_in", &[])
                },
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
                node: AudioNode {
                    serial: 1593,
                    ..node(Direction::Input, "bluez_in", &[cubic(35)])
                },
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
                node: AudioNode {
                    serial: 29349,
                    ..node(Direction::Input, "bluez_in", &[cubic(35)])
                },
            },
        );
        assert_eq!(snapshot(&graph).source.unwrap().percent, 35);
    }

    #[test]
    fn removing_a_device_drops_its_routes() {
        let mut graph = graph_with(vec![
            Event::NodeAdded {
                id: 56,
                node: node(Direction::Output, "speaker", &[cubic(54)]),
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
                node: node(Direction::Output, "speaker", &[cubic(54)]),
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
    fn output_kind_comes_from_form_factor_then_node_name() {
        let ucm = "alsa_output.pci-0000_00_1f.3-platform-sof_sdw.HiFi__";
        assert_eq!(
            OutputKind::of_node(None, &format!("{ucm}Headphones__sink")),
            OutputKind::Headphones
        );
        assert_eq!(
            OutputKind::of_node(None, &format!("{ucm}HDMI1__sink")),
            OutputKind::Display
        );
        assert_eq!(
            OutputKind::of_node(None, &format!("{ucm}Speaker__sink")),
            OutputKind::Speaker
        );
        assert_eq!(
            OutputKind::of_node(Some("headset"), "bluez_output.00_11_22.1"),
            OutputKind::Headset
        );
        assert_eq!(
            OutputKind::of_node(Some("unheard-of"), "bluez_output.00_11_22.1"),
            OutputKind::Speaker
        );
    }
}
