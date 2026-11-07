use notulist_client::OutputProblem;
use sottomano_desktop_sources::voice_dictation_input::notulist_dbus_output_problems::NotulistOutputProblems;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for NotulistOutputProblems {
    fn name(&self) -> &'static str {
        "Typing output"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Keyboard(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Keyboard(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.problems.iter().any(OutputProblem::blocks_typing) {
            ReadingDisplayState::Failed
        } else {
            ReadingDisplayState::Warning
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.problems.len())
            .ok()
            .filter(|count| *count > 1)
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.problems.is_empty() {
            PanelVisibility::Hidden
        } else {
            PanelVisibility::Shown
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(self.problems.iter().map(output_problem_section))
    }
}

const fn output_problem_label(problem: &OutputProblem) -> &'static str {
    match problem {
        OutputProblem::TextFieldTypingUnavailable(_) => "No input method",
        OutputProblem::AnotherInputMethodOwnsSeat => "Seat taken",
        OutputProblem::VirtualKeyboardTypingUnavailable(_) => "Virtual keyboard failed",
        OutputProblem::TranscriptDiscardedWithoutTarget => "Transcript discarded",
    }
}

fn output_problem_section(problem: &OutputProblem) -> Section {
    Section::dictionary([LabeledValue::new("Details", problem.to_string())])
        .titled(output_problem_label(problem))
}

#[cfg(test)]
mod tests {
    use notulist_client::{InputMethodError, VirtualKeyboardError};

    use super::*;
    use crate::group_items::group_item_reading::PanelVisibility;

    #[test]
    fn no_problems_stay_off_the_panel() {
        let quiet = NotulistOutputProblems {
            problems: Vec::new(),
        };
        assert_eq!(quiet.panel_visibility(), PanelVisibility::Hidden);
    }

    #[test]
    fn a_discarded_transcript_only_warns_but_a_missing_input_method_fails() {
        let discarded = NotulistOutputProblems {
            problems: vec![OutputProblem::TranscriptDiscardedWithoutTarget],
        };
        let blocked = NotulistOutputProblems {
            problems: vec![
                OutputProblem::TranscriptDiscardedWithoutTarget,
                OutputProblem::TextFieldTypingUnavailable(
                    InputMethodError::WaylandCompositorUnreachable,
                ),
            ],
        };
        assert_eq!(discarded.state(Clock::real()), ReadingDisplayState::Warning);
        assert_eq!(blocked.state(Clock::real()), ReadingDisplayState::Failed);
        assert_eq!(blocked.badge(), Some(2));
    }

    #[test]
    fn each_problem_gets_its_own_titled_section() {
        let problems = [
            OutputProblem::AnotherInputMethodOwnsSeat,
            OutputProblem::VirtualKeyboardTypingUnavailable(
                VirtualKeyboardError::VirtualKeyboardProtocolNotOffered,
            ),
        ];
        let titles: Vec<Option<String>> = problems
            .iter()
            .map(|problem| output_problem_section(problem).title().map(str::to_owned))
            .collect();
        assert_eq!(
            titles,
            [
                Some("Seat taken".to_owned()),
                Some("Virtual keyboard failed".to_owned())
            ]
        );
    }
}
