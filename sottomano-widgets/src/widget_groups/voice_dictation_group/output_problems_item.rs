use notulist_client::{OutputProblem, VirtualKeyboardError};
use sottomano_sources::voice_dictation_input::notulist_dbus_output_problems::NotulistOutputProblems;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody, entry},
        widget_state::WidgetState,
    },
};

impl BarWidget for NotulistOutputProblems {
    fn name(&self) -> &'static str {
        "Typing output"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Keyboard(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Keyboard(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.problems.iter().any(OutputProblem::blocks_typing) {
            WidgetState::Failed
        } else {
            WidgetState::Warning
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
            entry("Exit code", exit_code.map(|code| code.to_string())),
            entry(
                "Stderr",
                Some(stderr.trim()).filter(|text| !text.is_empty()),
            ),
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
            .chain([KeyValue::new("Details", problem.to_string())]),
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
        assert_eq!(discarded.state(Clock::real()), WidgetState::Warning);
        assert_eq!(blocked.state(Clock::real()), WidgetState::Failed);
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
