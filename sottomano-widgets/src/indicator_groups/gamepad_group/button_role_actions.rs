use std::iter::once;

use notulist_client::DictationCommand;
use sottomano_compositor_model::compositor_requests::CompositorRequest;
use sottomano_desktop_sources::gamepad_controller_input::evdev_gamepad_devices::ButtonEdge;
use sottomano_toml_config::indicator_groups::gamepad_bindings::ButtonRole;

use super::Action;

fn spawned(command: impl IntoIterator<Item = impl Into<String>>) -> CompositorRequest {
    CompositorRequest::Spawn(command.into_iter().map(Into::into).collect())
}

pub(super) fn typed_key_command(key_chord: &str) -> Vec<String> {
    let (modifier_names, key_name) = key_chord.rsplit_once('+').unwrap_or(("", key_chord));
    let held_modifiers: Vec<&str> = modifier_names
        .split('+')
        .filter(|modifier_name| !modifier_name.is_empty())
        .collect();
    let pressed_modifiers = held_modifiers
        .iter()
        .flat_map(|modifier_name| ["-M", modifier_name]);
    let released_modifiers = held_modifiers
        .iter()
        .rev()
        .flat_map(|modifier_name| ["-m", modifier_name]);
    once("wtype")
        .chain(pressed_modifiers)
        .chain(["-k", key_name])
        .chain(released_modifiers)
        .map(str::to_owned)
        .collect()
}

pub(super) fn role_request(role: ButtonRole<'_>, edge: ButtonEdge) -> Option<Action> {
    match (role, edge) {
        (ButtonRole::PushToTalk, ButtonEdge::Pressed) => {
            Some(Action::Dictation(DictationCommand::Start))
        }
        (ButtonRole::PushToTalk, ButtonEdge::Released) => {
            Some(Action::Dictation(DictationCommand::Stop))
        }
        (ButtonRole::TypedKey(key), ButtonEdge::Pressed) => {
            Some(Action::Compositor(spawned(typed_key_command(key))))
        }
        (ButtonRole::Compositor(action), ButtonEdge::Pressed) => {
            Some(Action::Compositor(CompositorRequest::from(action)))
        }
        (ButtonRole::Command(command), ButtonEdge::Pressed) => {
            Some(Action::Compositor(spawned(command.iter().cloned())))
        }
        (ButtonRole::PointerButton(_), _)
        | (
            ButtonRole::TypedKey(_) | ButtonRole::Compositor(_) | ButtonRole::Command(_),
            ButtonEdge::Released,
        ) => None,
    }
}
