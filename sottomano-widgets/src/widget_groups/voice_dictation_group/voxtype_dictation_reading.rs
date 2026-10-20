use sottomano_sources::voice_dictation_input::voxtype_daemon_state::{
    DictationActivity, VoxtypeDictation,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::{KeyValue, TooltipBody, entry},
        unit_switch::UnitSwitchFace,
        widget_state::WidgetState,
    },
};

pub const RECOGNIZER_DAEMON: UnitSwitchFace = UnitSwitchFace {
    name: "Speech recognizer",
    icon: Phosphor::Ear(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "load the speech model so dictation can start",
        turning_off: "unload the speech model and free the GPU",
    },
};

pub const TYPING_DAEMON: UnitSwitchFace = UnitSwitchFace {
    name: "Virtual keyboard",
    icon: Phosphor::Keyboard(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "start the virtual keyboard that types and corrects the transcript",
        turning_off: "stop the virtual keyboard; dictation falls back to the clipboard",
    },
};

const fn activity_text(activity: DictationActivity) -> &'static str {
    match activity {
        DictationActivity::Idle => "ready",
        DictationActivity::Recording => "listening",
        DictationActivity::Streaming => "listening and typing as you speak",
        DictationActivity::Transcribing => "transcribing",
        DictationActivity::Outputting => "typing the transcript",
        DictationActivity::Stopped => "recognizer not running",
    }
}

#[must_use]
pub fn press_does(dictation: &VoxtypeDictation) -> &'static str {
    if dictation.activity.listening() {
        "stop listening and finish the transcript"
    } else {
        "start dictating at the cursor"
    }
}

impl BarWidget for VoxtypeDictation {
    fn name(&self) -> &'static str {
        "Dictation"
    }

    fn icon(&self) -> Glyph {
        match self.activity {
            DictationActivity::Stopped => Phosphor::SubtitlesSlash(Weight::Regular),
            DictationActivity::Idle => Phosphor::Subtitles(Weight::Regular),
            DictationActivity::Recording | DictationActivity::Streaming => {
                Phosphor::Waveform(Weight::Regular)
            }
            DictationActivity::Transcribing => Phosphor::ChatCircleDots(Weight::Regular),
            DictationActivity::Outputting => Phosphor::CursorText(Weight::Regular),
        }
        .into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.activity {
            DictationActivity::Stopped => WidgetState::Off,
            DictationActivity::Idle => WidgetState::Idle,
            DictationActivity::Recording | DictationActivity::Streaming => WidgetState::Live,
            DictationActivity::Transcribing | DictationActivity::Outputting => WidgetState::Pending,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(
            [
                Some(KeyValue::new("Dictation", activity_text(self.activity))),
                entry("Model", self.setup.model.clone()),
                entry("Backend", self.setup.backend.clone()),
                entry("Microphone", self.setup.device.clone()),
            ]
            .into_iter()
            .flatten(),
        )
    }
}

pub struct CancelDictation;

impl BarWidget for CancelDictation {
    fn name(&self) -> &'static str {
        "Cancel dictation"
    }

    fn icon(&self) -> Glyph {
        Phosphor::XCircle(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}
