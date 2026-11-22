use std::{
    collections::HashMap,
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
    main_loop::MainLoopRc,
    registry::RegistryRc,
};
use tokio::sync::{oneshot, watch};
use tokio_stream::{self as stream, StreamExt as _, wrappers::WatchStream};

use self::{
    graph_proxy_listeners::track_global,
    level_command_writes::apply_level_command,
    live_audio_session::{GraphEnumeration, LiveAudioGraph},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::{Availability, AvailabilityStreams},
    system_protocol_clients::{
        inotify_file_presence::path_present, protocol_peer_reconnects::subscribed,
    },
};

mod audio_graph_state;
mod graph_proxy_listeners;
mod level_command_writes;
mod live_audio_session;
mod output_device_kind;
mod peak_level_meter;
mod spa_pod_levels;

pub use output_device_kind::OutputKind;
pub use peak_level_meter::Peaks;

const REPLY_TIMEOUT: Duration = Duration::from_millis(750);
const DEFAULT_REMOTE: &str = "pipewire-0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MuteState {
    #[default]
    Audible,
    Muted,
}

impl MuteState {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            MuteState::Audible => MuteState::Muted,
            MuteState::Muted => MuteState::Audible,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            MuteState::Audible => "active",
            MuteState::Muted => "muted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelLevels {
    pub percent: i64,
    pub mute_state: MuteState,
    pub device: DeviceDescription,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceDescription {
    Described(String),
    Undescribed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DefaultChannel {
    Resolved(ChannelLevels),
    #[default]
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAudio {
    Silent,
    Audible,
    Muted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioState {
    pub sink: DefaultChannel,
    pub source: DefaultChannel,
    pub output_kind: OutputKind,
    pub windows: Arc<HashMap<i32, WindowAudio>>,
}

impl AudioState {
    fn channel(&self, direction: Direction) -> &DefaultChannel {
        match direction {
            Direction::Output => &self.sink,
            Direction::Input => &self.source,
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
    pub fn request_level_change(&self, target: Target, kind: CommandKind) -> Option<i64> {
        let (reply, outcome) = sync_channel(1);
        self.sender
            .send(Command {
                target,
                kind,
                reply,
            })
            .inspect_err(|_| {
                tracing::warn!("audio: pipewire loop is gone, cannot send {kind:?}");
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

#[derive(Debug, Clone, thiserror::Error)]
pub enum AudioConnectError {
    #[error("cannot start the pipewire thread: {0}")]
    ThreadNotStarted(io::ErrorKind),
    #[error("pipewire thread ended before connecting")]
    ThreadEndedBeforeConnecting,
    #[error("pipewire refused the connection: {0}")]
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

pub fn connection_events()
-> impl Stream<Item = Option<Result<AudioEvent, AudioConnectError>>> + Send {
    socket_present().while_available(|| subscribed((), |()| connect()).map(Some), || None)
}

fn socket_present() -> impl Stream<Item = Availability> + Send {
    use futures::{StreamExt, stream};
    let socket = pipewire_socket()
        .inspect_err(|error| tracing::warn!("audio: {error}"))
        .ok();
    stream::iter(socket).flat_map(path_present)
}

impl SnapshotSource for AudioState {
    type Error = AudioConnectError;
    type StreamInputs = ();

    const NAME: &'static str = "Audio connection";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        socket_present().while_available(
            || {
                subscribed((), |()| connect()).filter_map(|event| match event {
                    Ok(AudioEvent::State(state)) => Some(SnapshotDelivery::live_push(Ok(state))),
                    Ok(AudioEvent::Connected(_) | AudioEvent::Peaks(_)) => None,
                    Err(e) => Some(SnapshotDelivery::live_push(Err(e))),
                })
            },
            || SnapshotDelivery::SourceUnavailable,
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
pub enum CommandKind {
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
