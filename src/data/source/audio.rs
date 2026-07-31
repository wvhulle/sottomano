use std::{cell::RefCell, collections::HashMap, rc::Rc};

use iced::{Subscription, futures::SinkExt};
use pipewire::{
    context::Context,
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
        sys::{SPA_PARAM_Props, SPA_PROP_channelVolumes, SPA_PROP_mute, SPA_TYPE_OBJECT_Props},
        utils::dict::DictRef,
    },
    types::ObjectType,
};
use tokio::sync::watch;

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

#[derive(Clone)]
pub struct Commands {
    sender: pipewire::channel::Sender<Command>,
}

impl std::fmt::Debug for Commands {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Commands")
    }
}

impl Commands {
    pub fn set_volume(&self, input: bool, percent: i64) -> bool {
        self.request(input, CommandKind::SetVolume(percent.clamp(0, 100)))
    }

    pub fn toggle_mute(&self, input: bool) -> bool {
        self.request(input, CommandKind::ToggleMute)
    }

    fn request(&self, input: bool, kind: CommandKind) -> bool {
        self.sender.send(Command { input, kind }).is_ok()
    }
}

pub fn subscription() -> Subscription<AudioEvent> {
    Subscription::run_with("audio", |_| {
        iced::stream::channel(
            16,
            move |mut output: iced::futures::channel::mpsc::Sender<AudioEvent>| async move {
                let (states, mut updates) = watch::channel(AudioState::default());
                let (commands, requests) = pipewire::channel::channel::<Command>();
                if std::thread::Builder::new()
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
            },
        )
    })
}

#[derive(Debug, Clone, Copy)]
struct Command {
    input: bool,
    kind: CommandKind,
}

#[derive(Debug, Clone, Copy)]
enum CommandKind {
    SetVolume(i64),
    ToggleMute,
}

#[derive(Default)]
struct NodeState {
    name: String,
    description: String,
    channel_volumes: Vec<f32>,
    mute: bool,
    proxy: Option<Node>,
    #[allow(dead_code)]
    listener: Option<NodeListener>,
}

#[derive(Default)]
struct Graph {
    nodes: HashMap<u32, NodeState>,
    default_sink: Option<String>,
    default_source: Option<String>,
    #[allow(dead_code)]
    defaults: Option<Metadata>,
    #[allow(dead_code)]
    defaults_listener: Option<MetadataListener>,
}

fn run(states: &watch::Sender<AudioState>, commands: pipewire::channel::Receiver<Command>) {
    pipewire::init();
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
    let states = Rc::new(states.clone());

    let _command_receiver = commands.attach(main_loop.loop_(), {
        let graph = graph.clone();
        move |command| apply(&graph.borrow(), command)
    });

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
                graph.borrow_mut().nodes.remove(&id);
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
        ObjectType::Node => {
            let class = props.get("media.class").unwrap_or_default();
            if class != "Audio/Sink" && class != "Audio/Source" {
                return;
            }
            let Ok(node) = registry.bind::<Node, _>(global) else {
                return;
            };
            node.subscribe_params(&[ParamType::Props]);
            let id = global.id;
            let listener = node
                .add_listener_local()
                .param({
                    let graph = graph.clone();
                    let states = states.clone();
                    move |_, _, _, _, param| {
                        let Some(update) = param.and_then(parse_props) else {
                            return;
                        };
                        if let Some(node) = graph.borrow_mut().nodes.get_mut(&id) {
                            if let Some(volumes) = update.volumes {
                                node.channel_volumes = volumes;
                            }
                            if let Some(mute) = update.mute {
                                node.mute = mute;
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
                    proxy: Some(node),
                    listener: Some(listener),
                    ..NodeState::default()
                },
            );
        }
        ObjectType::Metadata => {
            if props.get("metadata.name") != Some("default") {
                return;
            }
            let Ok(metadata) = registry.bind::<Metadata, _>(global) else {
                return;
            };
            let listener = metadata
                .add_listener_local()
                .property({
                    let graph = graph.clone();
                    let states = states.clone();
                    move |_, key, _, value| {
                        if let Some(key) = key {
                            let name = value.and_then(default_node_name);
                            {
                                let mut graph = graph.borrow_mut();
                                match key {
                                    "default.audio.sink" => graph.default_sink = name,
                                    "default.audio.source" => graph.default_source = name,
                                    _ => {}
                                }
                            }
                            publish(&graph.borrow(), &states);
                        }
                        0
                    }
                })
                .register();
            let mut graph = graph.borrow_mut();
            graph.defaults = Some(metadata);
            graph.defaults_listener = Some(listener);
        }
        _ => {}
    }
}

fn apply(graph: &Graph, command: Command) {
    let default = if command.input {
        graph.default_source.as_deref()
    } else {
        graph.default_sink.as_deref()
    };
    let Some(node) = default.and_then(|name| graph.nodes.values().find(|node| node.name == name))
    else {
        return;
    };
    let Some(proxy) = &node.proxy else {
        return;
    };
    let channels = node.channel_volumes.len().max(1);
    let property = match command.kind {
        CommandKind::SetVolume(percent) => {
            #[allow(clippy::cast_precision_loss)]
            let cubic = (percent as f32 / 100.0).powi(3);
            Property {
                key: SPA_PROP_channelVolumes,
                flags: PropertyFlags::empty(),
                value: Value::ValueArray(ValueArray::Float(vec![cubic; channels])),
            }
        }
        CommandKind::ToggleMute => Property {
            key: SPA_PROP_mute,
            flags: PropertyFlags::empty(),
            value: Value::Bool(!node.mute),
        },
    };
    if let Some(bytes) = props_pod(vec![property])
        && let Some(pod) = Pod::from_bytes(&bytes)
    {
        proxy.set_param(ParamType::Props, 0, pod);
    }
}

fn publish(graph: &Graph, states: &watch::Sender<AudioState>) {
    states.send_replace(AudioState {
        sink: reading(graph, graph.default_sink.as_deref()),
        source: reading(graph, graph.default_source.as_deref()),
    });
}

fn reading(graph: &Graph, name: Option<&str>) -> Option<Reading> {
    let name = name?;
    let node = graph.nodes.values().find(|node| node.name == name)?;
    let percent = volume_percent(&node.channel_volumes);
    Some(Reading {
        percent,
        muted: node.mute || percent == 0,
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

fn parse_props(pod: &Pod) -> Option<PropsUpdate> {
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?;
    let Value::Object(object) = value else {
        return None;
    };
    let volumes = object
        .properties
        .iter()
        .find_map(|property| match &property.value {
            Value::ValueArray(ValueArray::Float(values))
                if property.key == SPA_PROP_channelVolumes =>
            {
                Some(values.clone())
            }
            _ => None,
        });
    let mute = object
        .properties
        .iter()
        .find_map(|property| match property.value {
            Value::Bool(value) if property.key == SPA_PROP_mute => Some(value),
            _ => None,
        });
    Some(PropsUpdate { volumes, mute })
}

fn props_pod(properties: Vec<Property>) -> Option<Vec<u8>> {
    let object = Value::Object(Object {
        type_: SPA_TYPE_OBJECT_Props,
        id: SPA_PARAM_Props,
        properties,
    });
    let (cursor, _) = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &object).ok()?;
    Some(cursor.into_inner())
}
