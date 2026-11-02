use notulist_client::{OutputProblem, VirtualKeyboardError};
use sottomano_sources::voice_dictation_input::notulist_dbus_output_problems::NotulistOutputProblems;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        tooltip_body::{Section, TooltipBody},
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

    fn visible_in_bar(&self) -> bool {
        !self.problems.is_empty()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections(self.problems.iter().map(output_problem_section))
    }
}

const fn output_problem_label(problem: &OutputProblem) -> &'static str {
    match problem {
        OutputProblem::TextFieldTypingUnavailable(_) => "No input method",
        OutputProblem::AnotherInputMethodOwnsSeat => "Seat taken",
        OutputProblem::DotoolTypingUnavailable(_) => "Virtual keyboard failed",
        OutputProblem::TranscriptDiscardedWithoutTarget => "Transcript discarded",
    }
}

fn output_problem_section(problem: &OutputProblem) -> Section {
    let field_rows = match problem {
        OutputProblem::DotoolTypingUnavailable(VirtualKeyboardError::DotoolExitedWithError {
            exit_code,
            stderr,
        }) => vec![
            exit_code
                .map(|code| code.to_string())
                .map(|exit_code| LabeledValue::new("Exit code", exit_code)),
            Some(stderr.trim())
                .filter(|text| !text.is_empty())
                .map(|stderr_text| LabeledValue::new("Stderr", stderr_text)),
        ],
        OutputProblem::TextFieldTypingUnavailable(_)
        | OutputProblem::AnotherInputMethodOwnsSeat
        | OutputProblem::DotoolTypingUnavailable(VirtualKeyboardError::DotoolNotRunnable)
        | OutputProblem::TranscriptDiscardedWithoutTarget => Vec::new(),
    };
    Section::dictionary(
        field_rows
            .into_iter()
            .flatten()
            .chain([LabeledValue::new("Details", problem.to_string())]),
    )
    .titled(output_problem_label(problem))
}

#[cfg(test)]
mod tests {
    use notulist_client::InputMethodError;

    use super::*;

    #[test]
    fn no_problems_stay_out_of_the_bar() {
        let quiet = NotulistOutputProblems {
            problems: Vec::new(),
        };
        assert!(!quiet.visible_in_bar());
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
            OutputProblem::DotoolTypingUnavailable(VirtualKeyboardError::DotoolExitedWithError {
                exit_code: Some(1),
                stderr: "uinput: permission denied\n".to_owned(),
            }),
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
