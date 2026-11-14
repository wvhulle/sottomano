pub mod sqlite_compositor_styling;
pub mod switch_heat_borders;

use std::{io, sync::Arc};

use iced::Task;
use sottomano_desktop_sinks::compositor_window_requests::compositor_rule_files::{
    CompositorRulesFile, RulesSaveError, SaveStep,
};
use sottomano_desktop_sources::{
    compositor_window_ipc::{
        compositor_connection::CompositorKind,
        compositor_styling_model::{
            AppWindowLayout, CompositorStyling, ControlSupport, HeatedWindow, ShellBlurDepth,
            StylingControl,
        },
    },
    sqlite_state_store::StoreError,
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    group_items::{
        aligned_quantity::AlignedQuantity,
        group_item::GroupItem,
        slider_control::{
            Action as SliderAction, ControlMessage, ControlWriteFailure, SliderControl,
        },
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard, SourcedSections},
        },
    },
    widget_groups::{
        compositor_styling_group::switch_heat_borders::{SwitchHeatBorders, SwitchHeatVisibility},
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

impl From<ShellBlurDepth> for SwitchPosition {
    fn from(blur_depth: ShellBlurDepth) -> Self {
        match blur_depth {
            ShellBlurDepth::WallpaperOnly => SwitchPosition::On,
            ShellBlurDepth::WindowsBehind => SwitchPosition::Off,
        }
    }
}

fn xray_toggle(blur_depth: ShellBlurDepth) -> IconToggle {
    IconToggle::new(
        "X-ray",
        Phosphor::Eye(Weight::Regular),
        blur_depth,
        SwitchPresses {
            turning_on: "blur only the wallpaper, hiding windows behind the shell",
            turning_off: "blur the windows behind the shell too",
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorStylingTarget {
    CornerRadius,
    Gaps,
    Xray,
    SwitchHeat,
}

impl From<CompositorStylingTarget> for HoverTarget {
    fn from(target: CompositorStylingTarget) -> Self {
        HoverTarget::CompositorStyling(target)
    }
}

#[derive(Debug, Clone)]
pub enum CompositorStylingMessage {
    Expansion(WidgetGroupExpansionMessage),
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

fn compositor_face(compositor: Option<CompositorKind>) -> IconGlyph {
    match compositor.map(CompositorKind::name) {
        Some("niri") => SimpleIcon::niri.into(),
        Some("sway") => SimpleIcon::Sway.into(),
        Some("Hyprland") => SimpleIcon::Hyprland.into(),
        _ => Phosphor::AppWindow(Weight::Regular).into(),
    }
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
    expansion: WidgetGroupExpansion,
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
            expansion: WidgetGroupExpansion::default(),
            rules_file,
            blur_depth: defaults.blur_depth,
            corner_radius: SliderControl::new(
                "Window corner radius",
                0.0..=24.0,
                CompositorStylingTarget::CornerRadius,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            gaps: SliderControl::new(
                "Window gaps",
                0.0..=48.0,
                CompositorStylingTarget::Gaps,
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
                sqlite_compositor_styling::load(self.rules_file.config_home().to_path_buf()),
                CompositorStylingMessage::StylingLoaded,
            ),
        ])
    }

    #[must_use]
    pub fn app_window_layout(&self) -> AppWindowLayout {
        AppWindowLayout {
            window_corner_radius_px: self.corner_radius.synced_value.reported_value(),
            gaps_px: self.gaps.synced_value.reported_value(),
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
            window_corner_radius_px: self.corner_radius.synced_value.shown_value(),
            gaps_px: self.gaps.synced_value.shown_value(),
        }
    }

    fn requested_styling(&self) -> CompositorStyling {
        CompositorStyling {
            blur_depth: self.blur_depth,
            layout: self.requested_layout(),
        }
    }

    fn control_support(&self, control: StylingControl) -> ControlSupport {
        self.rules_file
            .compositor()
            .map_or(ControlSupport::Available, |compositor| {
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
        self.corner_radius.update(ControlMessage::Reading(
            styling.layout.window_corner_radius_px,
        ));
        self.gaps
            .update(ControlMessage::Reading(styling.layout.gaps_px));
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

    #[must_use]
    pub fn hover_card_for(&self, target: CompositorStylingTarget, clock: Clock) -> HoverCard {
        let control = match target {
            CompositorStylingTarget::CornerRadius => self
                .corner_radius
                .readout_hover_card("Window corner radius"),
            CompositorStylingTarget::Gaps => self.gaps.readout_hover_card("Window gaps"),
            CompositorStylingTarget::Xray => xray_toggle(self.blur_depth).switch_hover_card(clock),
            CompositorStylingTarget::SwitchHeat => {
                self.switch_heat.toggle().switch_hover_card(clock)
            }
        };
        control.with_groups(self.rules_file.unwritable().map(|unwritable| {
            SourcedSections::delivered(
                Section::dictionary([LabeledValue::new(
                    ValueCell::Path(unwritable.path.to_path_buf()),
                    unwritable.kind.to_string(),
                )
                .level(Severity::Critical)])
                .titled("The compositor rules are not written")
                .into(),
                DeliveryTiming::SingleRead,
            )
        }))
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

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, CompositorStylingMessage> {
        let styling_items = [
            (
                StylingControl::WindowCornerRadius,
                GroupItem::slider(
                    &self.corner_radius,
                    config,
                    CompositorStylingMessage::CornerRadius,
                )
                .aligned_as(AlignedQuantity::CornerRadius),
            ),
            (
                StylingControl::WindowGaps,
                GroupItem::slider(&self.gaps, config, CompositorStylingMessage::Gaps),
            ),
            (
                StylingControl::ShellBlurDepth,
                GroupItem::toggle(
                    xray_toggle(self.blur_depth),
                    CompositorStylingTarget::Xray,
                    CompositorStylingMessage::ToggleXray,
                ),
            ),
            (
                StylingControl::SwitchHeatBorders,
                GroupItem::toggle(
                    self.switch_heat.toggle(),
                    CompositorStylingTarget::SwitchHeat,
                    CompositorStylingMessage::ToggleSwitchHeat,
                ),
            ),
        ];
        WidgetGroupContent::expandable(
            WidgetGroupFamily::CompositorStyling,
            &self.expansion,
            CompositorStylingMessage::Expansion,
            styling_items
                .into_iter()
                .filter(|(control, _)| self.control_support(*control) == ControlSupport::Available)
                .map(|(_, styling_item)| styling_item),
        )
        .face(WidgetGroupFace::Icon {
            glyph: compositor_face(self.rules_file.compositor()),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use sottomano_desktop_sinks::compositor_window_requests::compositor_rule_files::CompositorRulesFile;
    use sottomano_desktop_sources::compositor_window_ipc::compositor_styling_model::{
        AppWindowLayout, CompositorStyling, ShellBlurDepth,
    };

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
