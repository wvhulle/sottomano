use chrono::Locale;
use notulist_client::OutputProblem;
use sottomano_desktop_sources::{
    SnapshotSource, voice_dictation_input::notulist_dbus_output_problems::NotulistOutputProblems,
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(crate) const NOTULIST_OUTPUT_PROBLEMS_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Keyboard(Weight::Regular));

pub fn notulist_output_problems_reading_row(
    output: &NotulistOutputProblems,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let state = if output.problems.iter().any(OutputProblem::blocks_typing) {
        ReadingDisplayState::Failed
    } else {
        ReadingDisplayState::Warning
    };
    let panel_visibility = if output.problems.is_empty() {
        PanelVisibility::Hidden
    } else {
        PanelVisibility::Shown
    };
    let badge = match output.problems.len() {
        0 | 1 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Ungauged,
            badge,
        },
        panel_visibility,
        ..ReadingRow::glyph_only(
            NotulistOutputProblems::NAME,
            Phosphor::Keyboard(Weight::Regular).into(),
            state,
            Box::new(move |card_clock| notulist_output_problems_hover_card(output, card_clock)),
        )
    }
}

pub fn notulist_output_problems_hover_card(
    output: &NotulistOutputProblems,
    _clock: Clock,
) -> Vec<Section> {
    output
        .problems
        .iter()
        .map(output_problem_section)
        .collect::<Vec<_>>()
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
    Section::new(
        output_problem_label(problem),
        dictionary_table([LabeledValue::new("Details", problem.to_string())]),
    )
}

#[cfg(test)]
mod tests {
    use notulist_client::{InputMethodError, VirtualKeyboardError};

    use super::*;
    use crate::group_items::reading_row::PanelVisibility;

    #[test]
    fn no_problems_stay_off_the_panel() {
        let quiet = NotulistOutputProblems {
            problems: Vec::new(),
        };
        assert_eq!(
            notulist_output_problems_reading_row(&quiet, Clock::real(), Locale::POSIX)
                .panel_visibility,
            PanelVisibility::Hidden
        );
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
        let discarded_row =
            notulist_output_problems_reading_row(&discarded, Clock::real(), Locale::POSIX);
        let blocked_row =
            notulist_output_problems_reading_row(&blocked, Clock::real(), Locale::POSIX);
        assert_eq!(discarded_row.shown_state(), ReadingDisplayState::Warning);
        assert_eq!(blocked_row.shown_state(), ReadingDisplayState::Failed);
        assert_eq!(blocked_row.readout.badge, ReadoutBadge::Counted(2));
    }

    #[test]
    fn each_problem_gets_its_own_titled_section() {
        let problems = [
            OutputProblem::AnotherInputMethodOwnsSeat,
            OutputProblem::VirtualKeyboardTypingUnavailable(
                VirtualKeyboardError::VirtualKeyboardProtocolNotOffered,
            ),
        ];
        let titles: Vec<String> = problems
            .iter()
            .map(|problem| output_problem_section(problem).title().text().to_owned())
            .collect();
        assert_eq!(
            titles,
            [
                "Seat taken".to_owned(),
                "Virtual keyboard failed".to_owned()
            ]
        );
    }
}
