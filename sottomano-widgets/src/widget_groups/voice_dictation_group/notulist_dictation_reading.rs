use notulist_client::{DictationPhase, DictationShortcutBinding};
use sottomano_sources::voice_dictation_input::notulist_dbus_dictation::NotulistDictation;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::{KeyValue, TooltipBody},
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
        let phase_row = KeyValue::new("Dictation", phase_text(self.phase));
        let shortcut_rows = self.shortcuts.iter().map(|shortcut| {
            KeyValue::new(
                shortcut.description.clone(),
                shortcut_trigger_text(shortcut),
            )
        });
        TooltipBody::dictionary(std::iter::once(phase_row).chain(shortcut_rows))
    }
}

fn shortcut_trigger_text(shortcut: &DictationShortcutBinding) -> String {
    if shortcut.bound_trigger.is_empty() {
        readable_xdg_trigger(&shortcut.preferred_trigger)
    } else {
        shortcut.bound_trigger.clone()
    }
}

fn readable_xdg_trigger(trigger: &str) -> String {
    trigger
        .split('+')
        .map(readable_xdg_trigger_part)
        .collect::<Vec<_>>()
        .join("+")
}

fn readable_xdg_trigger_part(part: &str) -> String {
    match part {
        "LOGO" => "Super".to_owned(),
        "SHIFT" => "Shift".to_owned(),
        "CTRL" => "Ctrl".to_owned(),
        "ALT" => "Alt".to_owned(),
        key => key.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(preferred_trigger: &str, bound_trigger: &str) -> DictationShortcutBinding {
        DictationShortcutBinding {
            shortcut_id: "dictate-while-held".to_owned(),
            description: "Dictate while held".to_owned(),
            preferred_trigger: preferred_trigger.to_owned(),
            bound_trigger: bound_trigger.to_owned(),
        }
    }

    #[test]
    fn unbound_shortcut_shows_its_preferred_trigger_readably() {
        assert_eq!(
            shortcut_trigger_text(&shortcut("LOGO+SHIFT+v", "")),
            "Super+Shift+V"
        );
    }

    #[test]
    fn bound_shortcut_shows_the_desktop_trigger() {
        assert_eq!(
            shortcut_trigger_text(&shortcut("LOGO+v", "Meta+V")),
            "Meta+V"
        );
    }
}
