use std::io;

use pipewire::spa::{
    pod::{
        Object, Pod, Property, PropertyFlags, Value, ValueArray, deserialize::PodDeserializer,
        serialize::PodSerializer,
    },
    sys::{
        SPA_PARAM_Props, SPA_PARAM_ROUTE_device, SPA_PARAM_ROUTE_index, SPA_PARAM_ROUTE_info,
        SPA_PARAM_ROUTE_props, SPA_PARAM_ROUTE_save, SPA_PARAM_Route, SPA_PROP_channelVolumes,
        SPA_PROP_mute, SPA_TYPE_OBJECT_ParamRoute, SPA_TYPE_OBJECT_Props,
    },
    utils::dict::DictRef,
};

use super::{
    Direction, OutputKind,
    audio_graph_state::{AppStream, AudioNode, Levels, LevelsUpdate, RouteLevels, RoutePlacement},
    level_command_writes::LevelChange,
};
use crate::media_playback_audio::pipewire_audio::{MuteState, audio_graph_state::StreamActivity};

pub(super) fn node_direction(props: &DictRef) -> Option<Direction> {
    match props.get("media.class")? {
        "Audio/Sink" => Some(Direction::Output),
        "Audio/Source" => Some(Direction::Input),
        _ => None,
    }
}

pub(super) fn stream_pid(props: &DictRef) -> Option<i32> {
    props.get("application.process.id")?.parse().ok()
}

pub(super) fn stream_from_props(props: &DictRef) -> AppStream {
    AppStream {
        pid: stream_pid(props),
        stream_activity: StreamActivity::Idle,
        levels: Levels::default(),
    }
}

pub(super) fn node_from_props(props: &DictRef, direction: Direction) -> Option<AudioNode> {
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

pub(super) fn route_placement(props: &DictRef) -> Option<RoutePlacement> {
    Some(RoutePlacement {
        device: props.get("device.id")?.parse().ok()?,
        route_device: props.get("card.profile.device")?.parse().ok()?,
    })
}

pub(super) fn default_node_name(value: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()?
        .get("name")?
        .as_str()
        .map(str::to_owned)
}

pub(super) fn parse_props(pod: &Pod) -> Option<LevelsUpdate> {
    parse_object(pod, SPA_TYPE_OBJECT_Props)
        .as_ref()
        .map(levels_update)
}

pub(super) fn parse_route(pod: &Pod) -> Option<RouteLevels> {
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
        mute_state: mute_of(object),
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

fn mute_of(object: &Object) -> Option<MuteState> {
    match property_value(object, SPA_PROP_mute)? {
        Value::Bool(true) => Some(MuteState::Muted),
        Value::Bool(false) => Some(MuteState::Audible),
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

pub(super) fn level_property(change: &LevelChange) -> Property {
    let (key, value) = match change {
        LevelChange::Volumes(volumes) => (
            SPA_PROP_channelVolumes,
            Value::ValueArray(ValueArray::Float(volumes.clone())),
        ),
        LevelChange::Mute(mute_state) => {
            (SPA_PROP_mute, Value::Bool(*mute_state == MuteState::Muted))
        }
    };
    Property {
        key,
        flags: PropertyFlags::empty(),
        value,
    }
}

pub(super) fn props_object(properties: Vec<Property>) -> Value {
    Value::Object(Object {
        type_: SPA_TYPE_OBJECT_Props,
        id: SPA_PARAM_Props,
        properties,
    })
}

#[allow(clippy::cast_possible_wrap)]
pub(super) fn route_object(index: u32, route_device: u32, change: &LevelChange) -> Value {
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

pub(super) fn pod_bytes(value: &Value) -> Option<Vec<u8>> {
    let (cursor, _) = PodSerializer::serialize(io::Cursor::new(Vec::new()), value).ok()?;
    Some(cursor.into_inner())
}
