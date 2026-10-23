use notulist_client::{DictationPhase, DictationShortcutBinding};
use sottomano_sources::voice_dictation_input::{
    notulist_dbus_dictation::NotulistDictation, notulist_dbus_shortcuts::NotulistShortcuts,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::{KeyValue, Section, TooltipBody},
        unit_switch::UnitSwitchFace,
        widget_state::WidgetState,
    },
};

pub const RECOGNIZER_DAEMON: UnitSwitchFace = UnitSwitchFace {
    name: "Speech server",
    icon: Phosphor::Brain(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "load the speech model so dictation can start",
        turning_off: "unload the speech model and free the GPU",
    },
};

pub const INPUT_METHOD_DAEMON: UnitSwitchFace = UnitSwitchFace {
    name: "Input method",
    icon: Phosphor::Keyboard(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "start the input method that types and corrects the transcript",
        turning_off: "stop the input method; nothing gets typed",
    },
};

const fn phase_text(phase: DictationPhase) -> &'static str {
    match phase {
        DictationPhase::LoadingModel => "loading the speech model",
        DictationPhase::Idle => "ready",
        DictationPhase::Listening => "listening and typing as you speak",
        DictationPhase::Finishing => "finishing the transcript",
        DictationPhase::Failed => "last session failed",
    }
}

#[must_use]
pub fn press_does(dictation: &NotulistDictation) -> &'static str {
    match dictation.phase {
        DictationPhase::Listening => "stop listening and finish the transcript",
        DictationPhase::LoadingModel => "wait for the speech model to load",
        DictationPhase::Idle | DictationPhase::Finishing | DictationPhase::Failed => {
            "start dictating at the cursor"
        }
    }
}

impl BarWidget for NotulistDictation {
    fn name(&self) -> &'static str {
        "Dictation"
    }

    fn icon(&self) -> Glyph {
        match self.phase {
            DictationPhase::LoadingModel => Phosphor::HourglassMedium(Weight::Regular),
            DictationPhase::Idle => Phosphor::Microphone(Weight::Regular),
            DictationPhase::Listening => Phosphor::Record(Weight::Regular),
            DictationPhase::Finishing => Phosphor::CursorText(Weight::Regular),
            DictationPhase::Failed => Phosphor::MicrophoneSlash(Weight::Regular),
        }
        .into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.phase {
            DictationPhase::Idle => WidgetState::Idle,
            DictationPhase::Listening => WidgetState::Live,
            DictationPhase::LoadingModel | DictationPhase::Finishing => WidgetState::Pending,
            DictationPhase::Failed => WidgetState::Failed,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("Dictation", phase_text(self.phase))])
    }
}

#[must_use]
pub fn shortcut_sections(shortcuts: &NotulistShortcuts) -> Vec<Section> {
    let shortcut_rows = shortcuts
        .bindings
        .iter()
        .map(|binding| KeyValue::new(binding.description.as_str(), shortcut_trigger(binding)));
    vec![Section::dictionary(shortcut_rows).titled("Shortcuts")]
}

fn shortcut_trigger(binding: &DictationShortcutBinding) -> String {
    if binding.bound_trigger.is_empty() {
        format!("preferred: {}", binding.preferred_trigger)
    } else {
        binding.bound_trigger.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(preferred_trigger: &str, bound_trigger: &str) -> DictationShortcutBinding {
        DictationShortcutBinding {
            shortcut_id: "dictate-while-held".to_owned(),
            description: "Dictate while held".to_owned(),
            preferred_trigger: preferred_trigger.to_owned(),
            bound_trigger: bound_trigger.to_owned(),
        }
    }

    #[test]
    fn desktop_bound_trigger_shows_as_is() {
        assert_eq!(shortcut_trigger(&binding("LOGO+v", "Meta+V")), "Meta+V");
    }

    #[test]
    fn unbound_trigger_is_labelled_as_only_preferred() {
        assert_eq!(
            shortcut_trigger(&binding("LOGO+v", "")),
            "preferred: LOGO+v"
        );
    }
}
