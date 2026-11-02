pub mod niri_kdl_rules;
mod switch_heat_borders;

use std::io;

use iced::Task;
use sottomano_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    group_items::{
        aligned_quantity::AlignedQuantity,
        group_item::GroupItem,
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard, SourcedSections},
        },
    },
    widget_groups::{
        niri_window_group::{
            niri_kdl_rules::{
                AdoptedRules, HeatedWindow, NiriLayout, NiriRulesFile, NiriStyling, RulesSaveError,
                SaveStep, StoredRules,
            },
            switch_heat_borders::SwitchHeatBorders,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

fn xray_toggle(xray: bool) -> IconToggle {
    IconToggle::new(
        "X-ray",
        Phosphor::Eye(Weight::Regular),
        xray,
        SwitchPresses {
            turning_on: "blur only the wallpaper, hiding windows behind the shell",
            turning_off: "blur the windows behind the shell too",
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiriWindowTarget {
    CornerRadius,
    Gaps,
    Xray,
    SwitchHeat,
}

impl From<NiriWindowTarget> for HoverTarget {
    fn from(target: NiriWindowTarget) -> Self {
        HoverTarget::NiriWindow(target)
    }
}

#[derive(Debug, Clone)]
pub enum NiriWindowMessage {
    Expansion(WidgetGroupExpansionMessage),
    CornerRadius(ControlMessage),
    Gaps(ControlMessage),
    ToggleXray,
    ToggleSwitchHeat,
    SwitchHeatLoaded(bool),
    RulesLoaded(StoredRules),
    RulesWritten(Result<(), io::ErrorKind>),
}

#[derive(Debug, Clone, Copy)]
enum CommittedSlider {
    CornerRadius(f32),
    Gaps(f32),
}

impl CommittedSlider {
    fn persisted(self, ok: bool) -> NiriWindowMessage {
        match self {
            Self::CornerRadius(value) => {
                NiriWindowMessage::CornerRadius(ControlMessage::Persisted { value, ok })
            }
            Self::Gaps(value) => NiriWindowMessage::Gaps(ControlMessage::Persisted { value, ok }),
        }
    }
}

fn report_unsaved_rules(save_error: &RulesSaveError) {
    match save_error {
        RulesSaveError::Rendering(_) => tracing::warn!("niri rules: {save_error}"),
        RulesSaveError::NotYetRead | RulesSaveError::Unreadable(_) => {
            tracing::debug!("niri rules: {save_error}");
        }
    }
}

pub struct NiriWindowGroup {
    expansion: WidgetGroupExpansion,
    rules_file: NiriRulesFile,
    sliders_awaiting_write: Vec<CommittedSlider>,
    xray: bool,
    corner_radius: SliderControl,
    gaps: SliderControl,
    switch_heat: SwitchHeatBorders,
}

impl NiriWindowGroup {
    #[must_use]
    pub fn new(rules_file: NiriRulesFile) -> Self {
        let defaults = NiriStyling::default();
        let mut niri_window_group = Self {
            expansion: WidgetGroupExpansion::default(),
            rules_file,
            sliders_awaiting_write: Vec::new(),
            xray: defaults.xray,
            corner_radius: SliderControl::new(
                "Window corner radius",
                0.0..=24.0,
                NiriWindowTarget::CornerRadius,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            gaps: SliderControl::new(
                "Window gaps",
                0.0..=48.0,
                NiriWindowTarget::Gaps,
                Phosphor::Columns(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            switch_heat: SwitchHeatBorders::default(),
        };
        niri_window_group.adopt_styling(defaults);
        niri_window_group
    }

    pub fn load(&self) -> Task<NiriWindowMessage> {
        Task::batch([
            SwitchHeatBorders::load(),
            Task::perform(self.rules_file.read(), NiriWindowMessage::RulesLoaded),
        ])
    }

    #[must_use]
    pub fn niri_layout(&self) -> NiriLayout {
        NiriLayout {
            window_corner_radius_px: self.corner_radius.settled_value(),
            gaps_px: self.gaps.settled_value(),
        }
    }

    #[must_use]
    pub fn niri_styling(&self) -> NiriStyling {
        NiriStyling {
            xray: self.xray,
            layout: self.niri_layout(),
        }
    }

    fn requested_styling(&self) -> NiriStyling {
        NiriStyling {
            xray: self.xray,
            layout: NiriLayout {
                window_corner_radius_px: self.corner_radius.current(),
                gaps_px: self.gaps.current(),
            },
        }
    }

    pub fn reheat(
        &mut self,
        config: &Config,
        heated_windows: impl FnOnce() -> Vec<HeatedWindow>,
    ) -> Task<NiriWindowMessage> {
        let styling = self.requested_styling();
        let saved = self.switch_heat.reheat(
            &mut self.rules_file,
            styling,
            &config.theme.palette,
            heated_windows,
        );
        self.follow_save(saved)
    }

    fn adopt_styling(&mut self, styling: NiriStyling) {
        self.xray = styling.xray;
        self.corner_radius.update(ControlMessage::Reading(
            styling.layout.window_corner_radius_px,
        ));
        self.gaps
            .update(ControlMessage::Reading(styling.layout.gaps_px));
    }

    fn adopt_rules(&mut self, stored: StoredRules) -> Task<NiriWindowMessage> {
        let AdoptedRules { styling, saved } =
            self.rules_file.adopt(stored, self.requested_styling());
        self.adopt_styling(styling);
        self.follow_save(saved)
    }

    fn save_styling(&mut self) -> Task<NiriWindowMessage> {
        let styling = self.requested_styling();
        let saved = self.rules_file.save(styling);
        self.follow_save(saved)
    }

    fn save_committed_slider(&mut self, committed: CommittedSlider) -> Task<NiriWindowMessage> {
        self.sliders_awaiting_write.push(committed);
        self.save_styling()
    }

    fn follow_save(
        &mut self,
        saved: Result<SaveStep, RulesSaveError>,
    ) -> Task<NiriWindowMessage> {
        match saved {
            Ok(SaveStep::StartWrite(rules_write)) => {
                Task::perform(rules_write.run(), NiriWindowMessage::RulesWritten)
            }
            Ok(SaveStep::WriteInFlight) => Task::none(),
            Ok(SaveStep::NothingInFlight) => self.acknowledge_sliders(true),
            Err(save_error) => {
                report_unsaved_rules(&save_error);
                self.acknowledge_sliders(false)
            }
        }
    }

    fn finish_rules_write(
        &mut self,
        written: Result<(), io::ErrorKind>,
    ) -> Task<NiriWindowMessage> {
        if let Err(kind) = written {
            tracing::warn!(
                "niri rules: writing {} failed: {kind}",
                self.rules_file.path().display()
            );
        }
        match self.rules_file.finish_write(written) {
            SaveStep::StartWrite(rules_write) => {
                Task::perform(rules_write.run(), NiriWindowMessage::RulesWritten)
            }
            SaveStep::WriteInFlight | SaveStep::NothingInFlight => {
                self.acknowledge_sliders(written.is_ok())
            }
        }
    }

    fn acknowledge_sliders(&mut self, ok: bool) -> Task<NiriWindowMessage> {
        Task::batch(
            self.sliders_awaiting_write
                .drain(..)
                .map(|committed| Task::done(committed.persisted(ok)))
                .collect::<Vec<_>>(),
        )
    }

    #[must_use]
    pub fn hover_card_for(&self, target: NiriWindowTarget, clock: Clock) -> HoverCard {
        let control = match target {
            NiriWindowTarget::CornerRadius => self
                .corner_radius
                .readout_hover_card("Window corner radius"),
            NiriWindowTarget::Gaps => self.gaps.readout_hover_card("Window gaps"),
            NiriWindowTarget::Xray => xray_toggle(self.xray).switch_hover_card(clock),
            NiriWindowTarget::SwitchHeat => self.switch_heat.toggle().switch_hover_card(clock),
        };
        control.with_groups(self.rules_file.unreadable().map(|unreadable| {
            SourcedSections::delivered(
                Section::dictionary([LabeledValue::new(
                    ValueCell::Path(unreadable.path.to_path_buf()),
                    unreadable.kind.to_string(),
                )
                .level(Severity::Critical)])
                .titled("Changes are not saved")
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
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: NiriWindowMessage) -> Task<NiriWindowMessage> {
        match msg {
            NiriWindowMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Task::none()
            }
            NiriWindowMessage::CornerRadius(control) => {
                let SliderAction::Commit(value) = self.corner_radius.update(control) else {
                    return Task::none();
                };
                self.save_committed_slider(CommittedSlider::CornerRadius(value))
            }
            NiriWindowMessage::Gaps(control) => {
                let SliderAction::Commit(value) = self.gaps.update(control) else {
                    return Task::none();
                };
                self.save_committed_slider(CommittedSlider::Gaps(value))
            }
            NiriWindowMessage::ToggleXray => {
                self.xray = !self.xray;
                self.save_styling()
            }
            NiriWindowMessage::ToggleSwitchHeat => self.switch_heat.flip(),
            NiriWindowMessage::SwitchHeatLoaded(shown) => {
                self.switch_heat.adopt_loaded(shown);
                Task::none()
            }
            NiriWindowMessage::RulesLoaded(stored) => self.adopt_rules(stored),
            NiriWindowMessage::RulesWritten(written) => self.finish_rules_write(written),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, NiriWindowMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::NiriWindow,
            &self.expansion,
            NiriWindowMessage::Expansion,
            [
                GroupItem::slider(&self.corner_radius, config, NiriWindowMessage::CornerRadius)
                    .aligned_as(AlignedQuantity::CornerRadius),
                GroupItem::slider(&self.gaps, config, NiriWindowMessage::Gaps),
                GroupItem::toggle(
                    xray_toggle(self.xray),
                    NiriWindowTarget::Xray,
                    NiriWindowMessage::ToggleXray,
                ),
                GroupItem::toggle(
                    self.switch_heat.toggle(),
                    NiriWindowTarget::SwitchHeat,
                    NiriWindowMessage::ToggleSwitchHeat,
                ),
            ],
        )
        .face(WidgetGroupFace::Icon {
            glyph: SimpleIcon::niri.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{NiriWindowGroup, NiriWindowMessage};
    use crate::widget_groups::niri_window_group::niri_kdl_rules::{
        NiriLayout, NiriRulesFile, NiriStyling, StoredRules,
    };

    const STORED_RULES: &str = r#"layer-rule {
    match namespace="^sottomano-panel$"
    background-effect {
        blur true
        xray true
    }
}
layout {
    gaps 5.0
}
window-rule {
    geometry-corner-radius 7.0
    clip-to-geometry true
}
"#;

    fn unloaded_group() -> NiriWindowGroup {
        NiriWindowGroup::new(NiriRulesFile::in_config_home(Path::new("/nonexistent")))
    }

    #[test]
    fn an_unloaded_group_reads_the_default_styling() {
        assert_eq!(unloaded_group().niri_styling(), NiriStyling::default());
    }

    #[test]
    fn loaded_rules_read_back_as_the_group_styling() {
        let mut niri_window_group = unloaded_group();
        let _ = niri_window_group.update(NiriWindowMessage::RulesLoaded(StoredRules::Text(
            STORED_RULES.to_owned(),
        )));
        assert_eq!(
            niri_window_group.niri_styling(),
            NiriStyling {
                xray: true,
                layout: NiriLayout {
                    window_corner_radius_px: 7.0,
                    gaps_px: 5.0,
                },
            }
        );
    }
}
