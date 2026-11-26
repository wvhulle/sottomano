pub mod sqlite_compositor_styling;
mod styling_group_indicators;
pub mod switch_heat_borders;

use std::{io, sync::Arc};

use iced::Task;
use sottomano_compositor_model::{
    compositor_backend::CompositorFeatureSupport,
    compositor_styling_model::{
        AppWindowLayout, CompositorStyling, HeatedWindow, ShellBlurDepth, StylingControl,
    },
};
use sottomano_desktop_sinks::compositor_rule_files::{
    CompositorRulesFile, RulesSaveError, SaveStep,
};
use sottomano_desktop_sources::sqlite_state_store::StoreError;
use sottomano_toml_config::Config;

use crate::{
    background_work::logged_failure_task,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            ExpansionChange, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        slider_control::{
            Action as SliderAction, ControlMessage, ControlWriteFailure, SliderControl,
        },
    },
    panel_menus::appearance_menu::compositor_styling_group::switch_heat_borders::{
        SwitchHeatBorders, SwitchHeatVisibility,
    },
};

#[derive(Debug, Clone)]
pub enum CompositorStylingMessage {
    Expansion(IndicatorGroupExpansionMessage),
    CornerRadius(ControlMessage),
    Gaps(ControlMessage),
    ToggleXray,
    ToggleSwitchHeat,
    SwitchHeatLoaded(SwitchHeatVisibility),
    StylingLoaded(CompositorStyling),
    RulesWritten(Result<(), io::ErrorKind>),
}

#[derive(Debug, Clone, Copy)]
enum CommittedSlider {
    CornerRadius(f32),
    Gaps(f32),
}

impl CommittedSlider {
    fn persisted(self, write_outcome: Result<(), ControlWriteFailure>) -> CompositorStylingMessage {
        match self {
            Self::CornerRadius(value) => {
                CompositorStylingMessage::CornerRadius(ControlMessage::Persisted {
                    value,
                    write_outcome,
                })
            }
            Self::Gaps(value) => CompositorStylingMessage::Gaps(ControlMessage::Persisted {
                value,
                write_outcome,
            }),
        }
    }
}

fn report_unsaved_rules(save_error: &RulesSaveError) {
    match save_error {
        RulesSaveError::Rendering(_) => tracing::warn!("compositor rules: {save_error}"),
        RulesSaveError::StylingNotLoaded => tracing::debug!("compositor rules: {save_error}"),
    }
}

fn written_outcome(written: Result<(), StoreError>) -> Result<(), ControlWriteFailure> {
    written.map_err(|store_error| ControlWriteFailure::Rejected(Arc::new(store_error)))
}

fn followed_rules_save(saved: Result<SaveStep, RulesSaveError>) -> Task<CompositorStylingMessage> {
    match saved {
        Ok(SaveStep::StartWrite(rules_write)) => {
            Task::perform(rules_write.run(), CompositorStylingMessage::RulesWritten)
        }
        Ok(SaveStep::WriteInFlight | SaveStep::NothingInFlight) => Task::none(),
        Err(save_error) => {
            report_unsaved_rules(&save_error);
            Task::none()
        }
    }
}

pub struct CompositorStylingGroup {
    expansion: IndicatorGroupExpansion,
    rules_file: CompositorRulesFile,
    blur_depth: ShellBlurDepth,
    corner_radius: SliderControl,
    gaps: SliderControl,
    switch_heat: SwitchHeatBorders,
}

impl CompositorStylingGroup {
    #[must_use]
    pub fn new(rules_file: CompositorRulesFile) -> Self {
        let defaults = CompositorStyling::default();
        let mut compositor_styling_group = Self {
            expansion: IndicatorGroupExpansion::default(),
            rules_file,
            blur_depth: defaults.blur_depth,
            corner_radius: SliderControl::new(
                "Window corner radius",
                0.0..=24.0,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            gaps: SliderControl::new(
                "Window gaps",
                0.0..=48.0,
                Phosphor::Columns(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            switch_heat: SwitchHeatBorders::default(),
        };
        compositor_styling_group.adopt_styling(defaults);
        compositor_styling_group
    }

    pub fn load(&self) -> Task<CompositorStylingMessage> {
        Task::batch([
            SwitchHeatBorders::load(),
            Task::perform(
                sqlite_compositor_styling::load(),
                CompositorStylingMessage::StylingLoaded,
            ),
        ])
    }

    #[must_use]
    pub fn app_window_layout(&self) -> AppWindowLayout {
        AppWindowLayout {
            window_corner_radius_px: self
                .corner_radius
                .expanding_slider
                .synced_value
                .reported_value(),
            gaps_px: self.gaps.expanding_slider.synced_value.reported_value(),
        }
    }

    #[must_use]
    pub fn compositor_styling(&self) -> CompositorStyling {
        CompositorStyling {
            blur_depth: self.blur_depth,
            layout: self.app_window_layout(),
        }
    }

    fn requested_layout(&self) -> AppWindowLayout {
        AppWindowLayout {
            window_corner_radius_px: self
                .corner_radius
                .expanding_slider
                .synced_value
                .shown_value(),
            gaps_px: self.gaps.expanding_slider.synced_value.shown_value(),
        }
    }

    fn requested_styling(&self) -> CompositorStyling {
        CompositorStyling {
            blur_depth: self.blur_depth,
            layout: self.requested_layout(),
        }
    }

    fn control_support(&self, control: StylingControl) -> CompositorFeatureSupport {
        self.rules_file
            .compositor()
            .map_or(CompositorFeatureSupport::Available, |compositor| {
                compositor.styling_support(control)
            })
    }

    pub fn reheat(
        &mut self,
        config: &Config,
        heated_windows: impl FnOnce() -> Vec<HeatedWindow>,
    ) -> Task<CompositorStylingMessage> {
        let styling = self.requested_styling();
        let saved = self.switch_heat.reheat(
            &mut self.rules_file,
            styling,
            &config.theme.palette,
            heated_windows,
        );
        followed_rules_save(saved)
    }

    fn adopt_styling(&mut self, styling: CompositorStyling) {
        self.blur_depth = styling.blur_depth;
        self.corner_radius.update(ControlMessage::Sample(
            styling.layout.window_corner_radius_px,
        ));
        self.gaps
            .update(ControlMessage::Sample(styling.layout.gaps_px));
    }

    fn adopt_loaded_styling(
        &mut self,
        styling: CompositorStyling,
    ) -> Task<CompositorStylingMessage> {
        self.adopt_styling(styling);
        let saved = self.rules_file.styling_loaded(styling);
        followed_rules_save(saved)
    }

    fn save_rules(&mut self) -> Task<CompositorStylingMessage> {
        let saved = self.rules_file.save(self.requested_styling());
        followed_rules_save(saved)
    }

    fn persist_committed_slider(
        &mut self,
        committed: CommittedSlider,
    ) -> Task<CompositorStylingMessage> {
        let layout_saved = Task::perform(
            sqlite_compositor_styling::save_layout(self.requested_layout()),
            move |written| committed.persisted(written_outcome(written)),
        );
        Task::batch([layout_saved, self.save_rules()])
    }

    fn finish_rules_write(
        &mut self,
        written: Result<(), io::ErrorKind>,
    ) -> Task<CompositorStylingMessage> {
        if let (Err(kind), Some(path)) = (written, self.rules_file.path()) {
            tracing::warn!(
                "compositor rules: writing {} failed: {kind}",
                path.display()
            );
        }
        match self.rules_file.finish_write(written) {
            SaveStep::StartWrite(rules_write) => {
                Task::perform(rules_write.run(), CompositorStylingMessage::RulesWritten)
            }
            SaveStep::WriteInFlight | SaveStep::NothingInFlight => Task::none(),
        }
    }
    fn close_sliders(&mut self) {
        self.corner_radius.close();
        self.gaps.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: CompositorStylingMessage) -> Task<CompositorStylingMessage> {
        match msg {
            CompositorStylingMessage::Expansion(menu) => {
                if self.expansion.update(menu) == ExpansionChange::JustCollapsed {
                    self.close_sliders();
                }
                Task::none()
            }
            CompositorStylingMessage::CornerRadius(control) => {
                let SliderAction::Commit(value) = self.corner_radius.update(control) else {
                    return Task::none();
                };
                self.persist_committed_slider(CommittedSlider::CornerRadius(value))
            }
            CompositorStylingMessage::Gaps(control) => {
                let SliderAction::Commit(value) = self.gaps.update(control) else {
                    return Task::none();
                };
                self.persist_committed_slider(CommittedSlider::Gaps(value))
            }
            CompositorStylingMessage::ToggleXray => {
                self.blur_depth = self.blur_depth.toggled();
                Task::batch([
                    logged_failure_task(
                        "shell blur depth: save".to_owned(),
                        sqlite_compositor_styling::save_blur_depth(self.blur_depth),
                    ),
                    self.save_rules(),
                ])
            }
            CompositorStylingMessage::ToggleSwitchHeat => self.switch_heat.flip(),
            CompositorStylingMessage::SwitchHeatLoaded(heat_visibility) => {
                self.switch_heat.adopt_loaded(heat_visibility);
                Task::none()
            }
            CompositorStylingMessage::StylingLoaded(styling) => self.adopt_loaded_styling(styling),
            CompositorStylingMessage::RulesWritten(written) => self.finish_rules_write(written),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use sottomano_compositor_model::compositor_styling_model::{
        AppWindowLayout, CompositorStyling, ShellBlurDepth,
    };
    use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;

    use super::{CompositorStylingGroup, CompositorStylingMessage};

    fn unloaded_group() -> CompositorStylingGroup {
        CompositorStylingGroup::new(CompositorRulesFile::in_config_home(
            Path::new("/nonexistent"),
            None,
        ))
    }

    #[test]
    fn loaded_styling_becomes_the_group_styling() {
        let loaded = CompositorStyling {
            blur_depth: ShellBlurDepth::WallpaperOnly,
            layout: AppWindowLayout {
                window_corner_radius_px: 7.0,
                gaps_px: 5.0,
            },
        };
        let mut compositor_styling_group = unloaded_group();
        let _ = compositor_styling_group.update(CompositorStylingMessage::StylingLoaded(loaded));
        assert_eq!(compositor_styling_group.compositor_styling(), loaded);
    }
}
