use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    env, fmt, io,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        mpsc::{SyncSender, sync_channel},
    },
    thread,
    time::Duration,
};

use futures::Stream;
use pipewire::{
    channel::{Receiver as CommandReceiver, Sender as CommandSender, channel as command_channel},
    context::ContextRc,
    core::CoreRc,
    device::{Device, DeviceChangeMask, DeviceInfoRef, DeviceListener},
    main_loop::MainLoopRc,
    metadata::{Metadata, MetadataListener},
    node::{Node, NodeInfoRef, NodeListener, NodeState},
    proxy::ProxyT,
    registry::{GlobalObject, Registry, RegistryRc},
    spa::{
        param::ParamType,
        pod::{
            Object, Pod, Property, PropertyFlags, Value, ValueArray, deserialize::PodDeserializer,
            serialize::PodSerializer,
        },
        sys::{
            SPA_PARAM_Props, SPA_PARAM_ROUTE_device, SPA_PARAM_ROUTE_index, SPA_PARAM_ROUTE_info,
            SPA_PARAM_ROUTE_props, SPA_PARAM_ROUTE_save, SPA_PARAM_Route, SPA_PROP_channelVolumes,
            SPA_PROP_mute, SPA_TYPE_OBJECT_ParamRoute, SPA_TYPE_OBJECT_Props,
        },
        utils::{dict::DictRef, result::AsyncSeq},
    },
    types::ObjectType,
};
use tokio::sync::{oneshot, watch};
use tokio_stream::{self as stream, StreamExt as _, wrappers::WatchStream};

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::{StreamOperators, connected_until_failure},
    system_protocol_clients::inotify_file_presence::path_present,
};

mod peak_level_meter;

pub use peak_level_meter::Peaks;

const REPLY_TIMEOUT: Duration = Duration::from_millis(750);
const DEFAULT_REMOTE: &str = "pipewire-0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelLevels {
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
    fn of_described_props(props: &DictRef, icon_key: &str) -> Option<Self> {
        props
            .get("device.form-factor")
            .and_then(Self::of_form_factor)
            .or_else(|| props.get(icon_key).and_then(Self::of_icon_name))
    }

    fn of_icon_name(icon_name: &str) -> Option<Self> {
        if icon_name.contains("headset") {
            Some(Self::Headset)
        } else if icon_name.contains("headphone") {
            Some(Self::Headphones)
        } else if icon_name.contains("display") || icon_name.contains("video") {
            Some(Self::Display)
        } else if icon_name.contains("speaker") {
            Some(Self::Speaker)
        } else {
            None
        }
    }

    fn of_port_type(port_type: &str) -> Option<Self> {
        match port_type {
            "headphones" => Some(Self::Headphones),
            "headset" | "handset" | "handsfree" => Some(Self::Headset),
            "hdmi" | "displayport" => Some(Self::Display),
            _ => None,
        }
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
    pub sink: Option<ChannelLevels>,
    pub source: Option<ChannelLevels>,
    pub output_kind: OutputKind,
    pub windows: Arc<HashMap<i32, WindowAudio>>,
}

impl AudioState {
    fn channel(&self, direction: Direction) -> Option<&ChannelLevels> {
        match direction {
            Direction::Output => self.sink.as_ref(),
            Direction::Input => self.source.as_ref(),
        }
    }
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
    #[must_use]
    pub fn set_volume(&self, target: Target, percent: i64) -> Option<i64> {
        self.request(target, CommandKind::SetVolume(percent.clamp(0, 100)))
    }

    #[must_use]
    pub fn adjust_volume(&self, target: Target, delta: i64) -> Option<i64> {
        self.request(target, CommandKind::AdjustVolume(delta))
    }

    #[must_use]
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
            })
            .ok()?;
        outcome
            .recv_timeout(REPLY_TIMEOUT)
            .inspect_err(|_| {
                tracing::warn!(
                    "audio: pipewire loop did not answer {kind:?} within {REPLY_TIMEOUT:?}"
                );
            })
            .ok()?
    }
}

#[derive(Debug, Clone)]
pub enum AudioConnectError {
    ThreadNotStarted(io::ErrorKind),
    ThreadEndedBeforeConnecting,
    PipewireRefused(Arc<pipewire::Error>),
}

impl From<io::Error> for AudioConnectError {
    fn from(error: io::Error) -> Self {
        Self::ThreadNotStarted(error.kind())
    }
}

impl From<oneshot::error::RecvError> for AudioConnectError {
    fn from(_: oneshot::error::RecvError) -> Self {
        Self::ThreadEndedBeforeConnecting
    }
}

impl From<pipewire::Error> for AudioConnectError {
    fn from(error: pipewire::Error) -> Self {
        Self::PipewireRefused(Arc::new(error))
    }
}

impl fmt::Display for AudioConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThreadNotStarted(kind) => write!(f, "cannot start the pipewire thread: {kind}"),
            Self::ThreadEndedBeforeConnecting => {
                write!(f, "pipewire thread ended before connecting")
            }
            Self::PipewireRefused(error) => write!(f, "pipewire refused the connection: {error}"),
        }
    }
}

pub fn connection_events()
-> impl Stream<Item = Option<Result<AudioEvent, AudioConnectError>>> + Send {
    socket_present().while_held(|| connected_until_failure(connect()).map(Some), None)
}

fn socket_present() -> impl Stream<Item = bool> + Send {
    use futures::{StreamExt, stream};
    let socket = pipewire_socket()
        .inspect_err(|e| tracing::warn!("audio: {e}"))
        .ok();
    stream::iter(socket).flat_map(path_present)
}

impl SnapshotSource for AudioState {
    type Error = AudioConnectError;
    type StreamInputs = ();

    const NAME: &'static str = "Audio";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        socket_present().while_held(
            || {
                connected_until_failure(connect()).filter_map(|event| match event {
                    Ok(AudioEvent::State(state)) => Some(SnapshotDelivery::live_push(Ok(state))),
                    Ok(AudioEvent::Connected(_) | AudioEvent::Peaks(_)) => None,
                    Err(e) => Some(SnapshotDelivery::live_push(Err(e))),
                })
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

async fn connect()
-> Result<impl Stream<Item = Result<AudioEvent, AudioConnectError>>, AudioConnectError> {
    let (states, updates) = watch::channel(AudioState::default());
    let (peaks, peak_updates) = watch::channel(Peaks::default());
    let (commands, requests) = command_channel::<Command>();
    let (connected, pipewire_connected) = oneshot::channel();
    thread::Builder::new()
        .name("pipewire-audio".to_owned())
        .spawn(move || run(&states, &peaks, requests, connected))?;
    pipewire_connected.await??;
    let changes = WatchStream::from_changes(updates)
        .map(AudioEvent::State)
        .merge(WatchStream::from_changes(peak_updates).map(AudioEvent::Peaks));
    Ok(
        stream::iter([AudioEvent::Connected(Commands { sender: commands })])
            .chain(changes)
            .map(Ok),
    )
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
}

#[derive(Debug, Clone, Copy)]
enum CommandKind {
    SetVolume(i64),
    AdjustVolume(i64),
    ToggleMute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Direction {
    Output,
    Input,
}

impl Direction {
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
    device_id: Option<u32>,
    props_kind: Option<OutputKind>,
    serial: u64,
    placement: Option<RoutePlacement>,
    levels: Levels,
}

#[derive(Debug)]
struct Route {
    index: u32,
    port_kind: Option<OutputKind>,
    levels: Levels,
}

#[derive(Debug)]
struct AppStream {
    pid: Option<i32>,
    running: bool,
    levels: Levels,
}

#[derive(Debug, Default)]
struct AudioGraph {
    nodes: HashMap<u32, AudioNode>,
    streams: HashMap<u32, AppStream>,
    routes: HashMap<u32, HashMap<u32, Route>>,
    device_kinds: HashMap<u32, OutputKind>,
    default_sink: Option<String>,
    default_source: Option<String>,
}

struct DefaultNode<'graph> {
    id: u32,
    node: &'graph AudioNode,
}

impl AudioGraph {
    fn add_node(&mut self, id: u32, node: AudioNode) {
        self.nodes.insert(id, node);
    }

    fn set_placement(&mut self, id: u32, placement: RoutePlacement) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.placement = Some(placement);
        }
    }

    fn set_kind_hint(&mut self, id: u32, kind: OutputKind) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.props_kind = Some(kind);
        }
    }

    fn set_device_kind(&mut self, id: u32, kind: OutputKind) {
        self.device_kinds.insert(id, kind);
    }

    fn merge_levels(&mut self, id: u32, update: LevelsUpdate) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.levels.merge(update);
        } else if let Some(stream) = self.streams.get_mut(&id) {
            stream.levels.merge(update);
        }
    }

    fn add_stream(&mut self, id: u32, stream: AppStream) {
        self.streams.insert(id, stream);
    }

    fn set_stream_info(&mut self, id: u32, pid: Option<i32>, running: bool) {
        if let Some(stream) = self.streams.get_mut(&id) {
            if pid.is_some() {
                stream.pid = pid;
            }
            stream.running = running;
        }
    }

    fn merge_route_levels(&mut self, device: u32, route: RouteLevels) {
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

    fn set_default(&mut self, change: DefaultNodeChange) {
        match change.direction {
            Direction::Output => self.default_sink = change.name,
            Direction::Input => self.default_source = change.name,
        }
    }

    fn remove_object(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.streams.remove(&id);
        self.routes.remove(&id);
        self.device_kinds.remove(&id);
    }

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

    fn default_node(&self, direction: Direction) -> Option<DefaultNode<'_>> {
        let name = self.default_name(direction)?;
        self.nodes
            .iter()
            .filter(|(_, node)| node.name == name)
            .max_by_key(|(id, node)| (node.serial, **id))
            .map(|(id, node)| DefaultNode { id: *id, node })
    }

    fn active_route(&self, node: &AudioNode) -> Option<&Route> {
        let placement = node.placement.as_ref()?;
        self.routes
            .get(&placement.device)?
            .get(&placement.route_device)
    }

    fn effective_levels<'graph>(&'graph self, node: &'graph AudioNode) -> &'graph Levels {
        self.active_route(node)
            .map_or(&node.levels, |route| &route.levels)
    }

    fn channel_levels(&self, direction: Direction) -> Option<ChannelLevels> {
        let DefaultNode { node, .. } = self.default_node(direction)?;
        let levels = self.effective_levels(node);
        let percent = levels.percent();
        Some(ChannelLevels {
            percent,
            muted: levels.mute || percent == 0,
            device: node.description.clone(),
        })
    }

    fn resolved_output_kind(&self, node: &AudioNode) -> OutputKind {
        self.active_route(node)
            .and_then(|route| route.port_kind)
            .or(node.props_kind)
            .or_else(|| self.device_kinds.get(&node.device_id?).copied())
            .unwrap_or_else(|| OutputKind::of_node_name(&node.name))
    }

    fn window_audio(&self) -> HashMap<i32, WindowAudio> {
        let mut by_pid: HashMap<i32, WindowAudio> = HashMap::new();
        for stream in self.streams.values() {
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

    fn audio_state(&self) -> AudioState {
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
struct DefaultNodeChange {
    direction: Direction,
    name: Option<String>,
}

#[derive(Debug, PartialEq)]
struct RouteLevels {
    index: u32,
    route_device: u32,
    port_kind: Option<OutputKind>,
    update: LevelsUpdate,
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

#[derive(Clone, Copy, Default)]
enum GraphEnumeration {
    #[default]
    NotStarted,
    ListingGlobals(AsyncSeq),
    AnsweringBinds(AsyncSeq),
    Settled,
}

impl GraphEnumeration {
    fn started(sync_result: Result<AsyncSeq, pipewire::Error>) -> Self {
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
struct BoundAudioSession {
    graph: AudioGraph,
    proxies: Proxies,
    enumeration: GraphEnumeration,
    reported_missing: HashSet<Direction>,
}

fn apply_level_command(
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
    let muted = !app_streams.is_empty() && app_streams.iter().all(|(_, stream)| stream.levels.mute);
    app_streams
        .iter()
        .filter_map(|(id, stream)| {
            let resolved = match kind {
                CommandKind::ToggleMute => Resolved {
                    change: LevelChange::Mute(!muted),
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
struct LiveAudioGraph {
    session: Rc<RefCell<BoundAudioSession>>,
    states: watch::Sender<AudioState>,
}

impl LiveAudioGraph {
    fn new(states: watch::Sender<AudioState>) -> Self {
        Self {
            session: Rc::default(),
            states,
        }
    }

    fn update(&self, change: impl FnOnce(&mut AudioGraph)) {
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

    fn settle_enumeration(&self, finished: AsyncSeq, core: &CoreRc) {
        let mut session = self.session.borrow_mut();
        session.enumeration = session.enumeration.advanced(finished, || core.sync(0));
        report_missing_defaults(&mut session);
    }

    fn remove_object(&self, id: u32) {
        {
            let mut session = self.session.borrow_mut();
            session.proxies.nodes.remove(&id);
            session.proxies.devices.remove(&id);
        }
        self.update(|graph| graph.remove_object(id));
    }
}

struct PipewireConnection {
    main_loop: MainLoopRc,
    core: CoreRc,
    registry: Rc<RegistryRc>,
}

impl PipewireConnection {
    fn open() -> Result<Self, pipewire::Error> {
        let main_loop = MainLoopRc::new(None)?;
        let core = ContextRc::new(&main_loop, None)?.connect_rc(None)?;
        let registry = Rc::new(core.get_registry_rc()?);
        Ok(Self {
            main_loop,
            core,
            registry,
        })
    }
}

fn run(
    states: &watch::Sender<AudioState>,
    peaks: &watch::Sender<Peaks>,
    commands: CommandReceiver<Command>,
    connected: oneshot::Sender<Result<(), pipewire::Error>>,
) {
    pipewire::init();
    let pipewire = match PipewireConnection::open() {
        Ok(pipewire) => pipewire,
        Err(e) => {
            let _ = connected.send(Err(e));
            return;
        }
    };
    let _ = connected.send(Ok(()));
    serve(
        &pipewire,
        &LiveAudioGraph::new(states.clone()),
        peaks,
        commands,
    );
    states.send_replace(AudioState::default());
    peaks.send_replace(Peaks::default());
}

fn serve(
    pipewire: &PipewireConnection,
    live_graph: &LiveAudioGraph,
    peaks: &watch::Sender<Peaks>,
    commands: CommandReceiver<Command>,
) {
    let PipewireConnection {
        main_loop,
        core,
        registry,
    } = pipewire;
    let _meters = peak_level_meter::attach(core, peaks);

    let _attached = commands.attach(main_loop.loop_(), {
        let live_graph = live_graph.clone();
        move |command: Command| {
            let outcome =
                apply_level_command(&live_graph.session.borrow(), command.target, command.kind);
            let _ = command.reply.send(outcome);
        }
    });

    let _core_listener = core
        .add_listener_local()
        .error(|id, _, res, message| {
            tracing::warn!("audio: pipewire object {id} reported an error ({res}): {message}");
        })
        .done({
            let core = core.clone();
            let live_graph = live_graph.clone();
            move |_, finished| live_graph.settle_enumeration(finished, &core)
        })
        .register();

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let registry = registry.clone();
            let live_graph = live_graph.clone();
            move |global| track_global(&registry, &live_graph, global)
        })
        .global_remove({
            let live_graph = live_graph.clone();
            move |id| live_graph.remove_object(id)
        })
        .register();

    live_graph.session.borrow_mut().enumeration = GraphEnumeration::started(core.sync(0));

    main_loop.run();
}

fn track_global(registry: &Registry, live_graph: &LiveAudioGraph, global: &GlobalObject<&DictRef>) {
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

fn bind<P: ProxyT>(registry: &Registry, global: &GlobalObject<&DictRef>, label: &str) -> Option<P> {
    registry
        .bind::<P, _>(global)
        .inspect_err(|e| tracing::warn!("audio: bind {label} {} failed: {e}", global.id))
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

fn default_node_change(key: Option<&str>, value: Option<&str>) -> Option<DefaultNodeChange> {
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
        device_id: props.get("device.id").and_then(|id| id.parse().ok()),
        props_kind: None,
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
        port_kind: route_info_value(&object, "port.type").and_then(OutputKind::of_port_type),
        update: nested_props(&object)
            .as_ref()
            .map(levels_update)
            .unwrap_or_default(),
    })
}

fn route_info_value<'object>(object: &'object Object, key: &str) -> Option<&'object str> {
    let Value::Struct(fields) = property_value(object, SPA_PARAM_ROUTE_info)? else {
        return None;
    };
    fields
        .get(1..)?
        .as_chunks::<2>()
        .0
        .iter()
        .find_map(|pair| match pair {
            [Value::String(name), Value::String(value)] if name == key => Some(value.as_str()),
            _ => None,
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
            device_id: None,
            props_kind: None,
            serial: 0,
            placement: None,
            levels: Levels {
                channel_volumes: volumes.to_vec(),
                mute: false,
            },
        }
    }

    fn session_with_undefaulted_sink(enumeration: GraphEnumeration) -> BoundAudioSession {
        let mut graph = AudioGraph::default();
        graph.add_node(56, node(Direction::Output, "speaker", &[cubic(54)]));
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
                    mute: None,
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
                    mute: Some(false),
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
                    mute: None,
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
    fn output_kind_helpers_map_pipewire_vocabulary() {
        let ucm = "alsa_output.pci-0000_00_1f.3-platform-sof_sdw.HiFi__";
        assert_eq!(
            OutputKind::of_node_name(&format!("{ucm}Headphones__sink")),
            OutputKind::Headphones
        );
        assert_eq!(
            OutputKind::of_node_name(&format!("{ucm}HDMI1__sink")),
            OutputKind::Display
        );
        assert_eq!(
            OutputKind::of_node_name("bluez_output.00_11_22.1"),
            OutputKind::Speaker
        );
        assert_eq!(
            OutputKind::of_form_factor("headset"),
            Some(OutputKind::Headset)
        );
        assert_eq!(OutputKind::of_form_factor("unheard-of"), None);
        assert_eq!(
            OutputKind::of_icon_name("audio-headset-bluetooth"),
            Some(OutputKind::Headset)
        );
        assert_eq!(
            OutputKind::of_icon_name("audio-headphones"),
            Some(OutputKind::Headphones)
        );
        assert_eq!(OutputKind::of_icon_name("audio-card-analog-usb"), None);
        assert_eq!(
            OutputKind::of_port_type("headphones"),
            Some(OutputKind::Headphones)
        );
        assert_eq!(OutputKind::of_port_type("speaker"), None);
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
