mod switch_heat_borders;

use iced::Task;
use sottomano_config::{Config, severity::Severity};
use sottomano_state::niri_rules::{HeatedWindow, NiriLayout, NiriRulesFile, NiriStyling};

pub use self::switch_heat_borders::ReheatDue;
use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::niri_window_group::switch_heat_borders::SwitchHeatBorders,
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::{Cell, KeyValue, Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

fn xray_toggle(styling: NiriStyling) -> IconToggle {
    IconToggle::new(
        "X-ray",
        Phosphor::Eye(Weight::Regular),
        styling.xray,
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
    Expansion(ExpansionMessage),
    CornerRadius(ControlMessage),
    Gaps(ControlMessage),
    ToggleXray,
    ToggleSwitchHeat,
    SwitchHeatLoaded(bool),
}

pub struct NiriWindowGroup {
    expansion: GroupExpansion,
    corner_radius: SliderControl,
    gaps: SliderControl,
    switch_heat: SwitchHeatBorders,
}

impl Default for NiriWindowGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
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
        }
    }
}

impl NiriWindowGroup {
    pub fn load() -> Task<NiriWindowMessage> {
        SwitchHeatBorders::load()
    }

    pub fn reheat(
        &self,
        due: ReheatDue,
        rules: &mut NiriRulesFile,
        config: &Config,
        heated_windows: impl FnOnce() -> Vec<HeatedWindow>,
    ) -> Task<NiriWindowMessage> {
        self.switch_heat
            .reheat(due, rules, &config.theme.palette, heated_windows)
    }

    pub fn adopt(&mut self, windows: NiriLayout) {
        self.corner_radius
            .update(ControlMessage::Reading(windows.window_corner_radius_px));
        self.gaps.update(ControlMessage::Reading(windows.gaps_px));
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: NiriWindowTarget,
        rules: &NiriRulesFile,
        clock: Clock,
    ) -> Tooltip {
        let control = match target {
            NiriWindowTarget::CornerRadius => {
                self.corner_radius.readout_tooltip("Window corner radius")
            }
            NiriWindowTarget::Gaps => self.gaps.readout_tooltip("Window gaps"),
            NiriWindowTarget::Xray => xray_toggle(rules.styling()).switch_tooltip(clock),
            NiriWindowTarget::SwitchHeat => self.switch_heat.toggle().switch_tooltip(clock),
        };
        control.with_sections(rules.unreadable().map(|unreadable| {
            Section::dictionary([KeyValue::new(
                Cell::Path(unreadable.path.to_path_buf()),
                unreadable.kind.to_string(),
            )
            .level(Severity::Critical)])
            .titled("Changes are not saved")
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

    pub fn update(
        &mut self,
        msg: NiriWindowMessage,
        rules: &mut NiriRulesFile,
    ) -> Task<NiriWindowMessage> {
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
                Task::perform(
                    rules.restyle(move |styling| styling.layout.window_corner_radius_px = value),
                    move |ok| {
                        NiriWindowMessage::CornerRadius(ControlMessage::Persisted { value, ok })
                    },
                )
            }
            NiriWindowMessage::Gaps(control) => {
                let SliderAction::Commit(value) = self.gaps.update(control) else {
                    return Task::none();
                };
                Task::perform(
                    rules.restyle(move |styling| styling.layout.gaps_px = value),
                    move |ok| NiriWindowMessage::Gaps(ControlMessage::Persisted { value, ok }),
                )
            }
            NiriWindowMessage::ToggleXray => {
                Task::future(rules.restyle(move |styling| styling.xray = !styling.xray)).discard()
            }
            NiriWindowMessage::ToggleSwitchHeat => self.switch_heat.flip(),
            NiriWindowMessage::SwitchHeatLoaded(shown) => {
                self.switch_heat.adopt_loaded(shown);
                Task::none()
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        styling: NiriStyling,
    ) -> GroupContent<'a, NiriWindowMessage> {
        GroupContent::new(
            &self.expansion,
            NiriWindowMessage::Expansion,
            [
                GroupItem::slider(&self.corner_radius, config, NiriWindowMessage::CornerRadius),
                GroupItem::slider(&self.gaps, config, NiriWindowMessage::Gaps),
                GroupItem::toggle(
                    xray_toggle(styling),
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
        .face(HandleFace::Icon {
            glyph: SimpleIcon::niri.into(),
            handle: GroupHandle::NiriWindow,
        })
    }
}
