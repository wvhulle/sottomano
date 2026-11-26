use sottomano_desktop_sources::ci_runner_tasks::RunnerAccess;

use super::{CiRunnerReference, CiRunsGroup, ci_task_indicator::WatchedCiRunner};
use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::Section,
};

pub(super) struct CiRunnerFailureIndicator<'view> {
    pub watched: WatchedCiRunner<'view>,
}

impl<'view> CiRunnerFailureIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.watched.runner_label.to_owned(),
            self.watched.kind.runner_icon(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.watched.status_text()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        self.watched
            .failure_section()
            .into_iter()
            .collect::<Vec<_>>()
    }

    pub(super) fn failing_runner(watched: WatchedCiRunner<'view>) -> Option<Self> {
        match watched.runner.access {
            RunnerAccess::Complete => None,
            RunnerAccess::EngineUnreadable(_) | RunnerAccess::AgentUnreachable(_) => {
                Some(Self { watched })
            }
        }
    }
}

impl CiRunsGroup {
    pub(super) fn runner_failure_row<'view>(
        &'view self,
        failure_indicator: CiRunnerFailureIndicator<'view>,
        failure_reference: CiRunnerReference,
    ) -> GroupIndicator<'view> {
        failure_indicator
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.runner_hover_card(&failure_reference, |watched_runner| {
                    CiRunnerFailureIndicator::failing_runner(watched_runner)
                        .map(|failing_indicator| failing_indicator.hover_card())
                })
            }))
    }
}
