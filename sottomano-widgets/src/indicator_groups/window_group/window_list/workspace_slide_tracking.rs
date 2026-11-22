use std::time::{Duration, Instant};

use corredo::transitions::{MotionState, SlideBand};
use iced::{Animation, animation::Easing};
use sottomano_desktop_sources::desktop_windows_apps::workspace_window_snapshot;
use sottomano_toml_config::screen_edges::Position;

use super::WindowList;
use crate::{clock::Clock, desktop_panels::panel_slot_drag::PanelSlotContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ShownWorkspace {
    pub(super) workspace_id: u64,
    pub(super) workspace_index: u8,
}

#[derive(Debug, Clone)]
pub(super) struct DepartureSlide {
    pub(super) departed: ShownWorkspace,
    pub(super) animation: Animation<bool>,
}

#[derive(Debug, Clone)]
pub(super) enum WorkspaceSlide {
    Settled,
    Sliding(DepartureSlide),
}

#[derive(Debug, Clone)]
pub(super) struct OutputWorkspaces {
    pub(super) shown: ShownWorkspace,
    pub(super) workspace_slide: WorkspaceSlide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum DepartingWorkspace {
    Absent,
    SlidingAway {
        departed: ShownWorkspace,
        slide_progress: f32,
    },
}

pub(super) fn started_slide(now: Instant, slide_duration: Duration) -> Animation<bool> {
    Animation::new(false)
        .easing(Easing::EaseOutCubic)
        .duration(slide_duration)
        .go(true, now)
}

pub(super) fn panel_slide_band(slot_context: PanelSlotContext<'_>) -> SlideBand {
    let frame_depth_px = slot_context.config.desktop_panels.height;
    let float_offset_px = slot_context.panel_float_offset_px;
    let window_gap_px = slot_context.compositor_window_gaps_px;
    match slot_context.edge {
        Position::Top => SlideBand::AlongTopEdge {
            frame_depth_px,
            float_offset_px,
            window_gap_px,
        },
        Position::Bottom => SlideBand::AlongBottomEdge {
            frame_depth_px,
            float_offset_px,
            window_gap_px,
        },
        Position::Left => SlideBand::OwnBounds,
    }
}

pub(super) fn workspace_index(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    workspace: u64,
) -> u8 {
    compositor
        .workspaces_for_output(output)
        .iter()
        .find(|indicator| indicator.id == workspace)
        .map_or(0, |indicator| indicator.idx)
}

impl WindowList {
    pub fn observe_active_workspaces(
        &mut self,
        compositor: &workspace_window_snapshot::Snapshot,
        clock: Clock,
        slide_duration: Duration,
    ) {
        compositor
            .active_workspace_ids_by_output()
            .for_each(|(output, workspace_id)| {
                let arrived = ShownWorkspace {
                    workspace_id,
                    workspace_index: workspace_index(compositor, output, workspace_id),
                };
                self.observe_output(output, arrived, clock.instant, slide_duration);
            });
    }

    fn observe_output(
        &mut self,
        output: &str,
        arrived: ShownWorkspace,
        now: Instant,
        slide_duration: Duration,
    ) {
        match self.output_workspaces.get_mut(output) {
            Some(observed) if observed.shown.workspace_id != arrived.workspace_id => {
                observed.workspace_slide = if slide_duration.is_zero() {
                    WorkspaceSlide::Settled
                } else {
                    WorkspaceSlide::Sliding(DepartureSlide {
                        departed: observed.shown,
                        animation: started_slide(now, slide_duration),
                    })
                };
                observed.shown = arrived;
            }
            Some(observed) => observed.shown = arrived,
            None => {
                self.output_workspaces.insert(
                    output.to_owned(),
                    OutputWorkspaces {
                        shown: arrived,
                        workspace_slide: WorkspaceSlide::Settled,
                    },
                );
            }
        }
    }

    #[must_use]
    pub fn workspace_slide_motion(&self, now: Instant) -> MotionState {
        let any_sliding = self.output_workspaces.values().any(|observed| {
            matches!(
                &observed.workspace_slide,
                WorkspaceSlide::Sliding(departure_slide)
                    if departure_slide.animation.is_animating(now)
            )
        });
        if any_sliding {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
    }

    pub fn settle_workspace_slides(&mut self, now: Instant) {
        self.output_workspaces.values_mut().for_each(|observed| {
            if let WorkspaceSlide::Sliding(departure_slide) = &observed.workspace_slide
                && !departure_slide.animation.is_animating(now)
            {
                observed.workspace_slide = WorkspaceSlide::Settled;
            }
        });
    }

    pub(super) fn departing_workspace(
        &self,
        output: &str,
        arriving_workspace: u64,
        now: Instant,
    ) -> DepartingWorkspace {
        match self.output_workspaces.get(output) {
            Some(OutputWorkspaces {
                workspace_slide: WorkspaceSlide::Sliding(departure_slide),
                ..
            }) if departure_slide.departed.workspace_id != arriving_workspace
                && departure_slide.animation.is_animating(now) =>
            {
                DepartingWorkspace::SlidingAway {
                    departed: departure_slide.departed,
                    slide_progress: departure_slide.animation.interpolate(0.0, 1.0, now),
                }
            }
            Some(_) | None => DepartingWorkspace::Absent,
        }
    }
}
