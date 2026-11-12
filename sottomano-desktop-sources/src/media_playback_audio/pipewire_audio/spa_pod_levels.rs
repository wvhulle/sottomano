use std::{fmt::Display, io, str::FromStr};

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
    parsed_prop(props, "application.process.id")
}

fn parsed_prop<Parsed: FromStr>(props: &DictRef, key: &str) -> Option<Parsed>
where
    Parsed::Err: Display,
{
    let raw_value = props.get(key)?;
    raw_value
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "audio: pipewire property {key} has invalid value {raw_value:?}: {parse_failure}"
            );
        })
        .ok()
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
        device_id: parsed_prop(props, "device.id"),
        props_kind: None,
        serial: parsed_prop(props, "object.serial").unwrap_or_default(),
        placement: None,
        levels: Levels::default(),
    })
}

pub(super) fn route_placement(props: &DictRef) -> Option<RoutePlacement> {
    Some(RoutePlacement {
        device: parsed_prop(props, "device.id")?,
        route_device: parsed_prop(props, "card.profile.device")?,
    })
}

pub(super) fn default_node_name(value: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(value)
        .inspect_err(|parse_failure| {
            tracing::warn!("audio: invalid default node metadata {value:?}: {parse_failure}");
        })
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
        index: route_number(&object, SPA_PARAM_ROUTE_index, "index")?,
        route_device: route_number(&object, SPA_PARAM_ROUTE_device, "device")?,
        port_kind: route_info_value(&object, "port.type").and_then(OutputKind::of_port_type),
        update: nested_props(&object)
            .as_ref()
            .map(levels_update)
            .unwrap_or_default(),
    })
}

fn route_number(object: &Object, key: u32, field_name: &str) -> Option<u32> {
    let signed_number = int_of(object, key)?;
    u32::try_from(signed_number)
        .inspect_err(|_| {
            tracing::warn!("audio: route {field_name} is negative: {signed_number}");
        })
        .ok()
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
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes())
        .inspect_err(|deserialize_failure| {
            tracing::warn!("audio: cannot deserialize pipewire pod: {deserialize_failure:?}");
        })
        .ok()?;
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
    let (cursor, _) = PodSerializer::serialize(io::Cursor::new(Vec::new()), value)
        .inspect_err(|serialize_failure| {
            tracing::warn!("audio: cannot serialize pipewire pod: {serialize_failure:?}");
        })
        .ok()?;
    Some(cursor.into_inner())
}
