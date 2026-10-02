use std::ops::RangeInclusive;

use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    state::{self, PanelLayout, WorkspacesPlacement},
    widget_groups::niri_group::switch_tooltip,
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::icon_toggle_item,
        tooltip_body::{Tooltip, row},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

pub enum Action {
    None,
    Run(Task<Message>),
    Reflow(Task<Message>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    Tint,
    Offset,
    Radius,
    Softness,
    Opacity,
}

impl Knob {
    const ALL: [Self; 5] = [
        Self::Tint,
        Self::Offset,
        Self::Radius,
        Self::Softness,
        Self::Opacity,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Tint => "Tint",
            Self::Offset => "Offset",
            Self::Radius => "Corner radius",
            Self::Softness => "Shadow softness",
            Self::Opacity => "Shadow opacity",
        }
    }

    fn target(self) -> HoverTarget {
        match self {
            Self::Tint => HoverTarget::BackgroundTint,
            Self::Offset => HoverTarget::FloatOffset,
            Self::Radius => HoverTarget::CornerRadius,
            Self::Softness => HoverTarget::ShadowSoftness,
            Self::Opacity => HoverTarget::ShadowOpacity,
        }
    }

    fn control(self) -> SliderControl {
        let (range, icon, readout): (RangeInclusive<f32>, Phosphor, fn(f32) -> String) = match self
        {
            Self::Tint => (0.0..=100.0, Phosphor::DropHalf(Weight::Regular), percent),
            Self::Offset => (0.0..=24.0, Phosphor::ArrowsIn(Weight::Regular), pixels),
            Self::Radius => (0.0..=16.0, Phosphor::FrameCorners(Weight::Regular), pixels),
            Self::Softness => (0.0..=60.0, Phosphor::Feather(Weight::Regular), pixels),
            Self::Opacity => (0.0..=100.0, Phosphor::CircleHalf(Weight::Regular), percent),
        };
        SliderControl::new(range, self.target(), icon)
            .icon_only()
            .readout_width(44.0)
            .readout(readout)
    }

    fn read(self, layout: &PanelLayout) -> f32 {
        match self {
            Self::Tint => layout.tint_factor * 100.0,
            Self::Offset => layout.float_offset_px,
            Self::Radius => layout.corner_radius.top_left,
            Self::Softness => layout.shadow.softness_px,
            Self::Opacity => layout.shadow.opacity * 100.0,
        }
    }

    fn write(self, layout: &mut PanelLayout, value: f32) -> Task<bool> {
        match self {
            Self::Tint => {
                layout.tint_factor = value / 100.0;
                Task::future(state::panel::save_tint_factor(layout.tint_factor))
            }
            Self::Offset => {
                layout.float_offset_px = value;
                Task::future(state::panel::save_float_offset(value))
            }
            Self::Radius => {
                layout.corner_radius = value.into();
                Task::future(state::panel::save_corner_radius(value))
            }
            Self::Softness => {
                layout.shadow.softness_px = value;
                Task::future(state::panel::save_shadow_softness(value))
            }
            Self::Opacity => {
                layout.shadow.opacity = value / 100.0;
                Task::future(state::panel::save_shadow_opacity(layout.shadow.opacity))
            }
        }
    }

    fn committed(self, persist: Task<Message>) -> Action {
        match self {
            Self::Offset => Action::Reflow(persist),
            Self::Radius | Self::Tint | Self::Softness | Self::Opacity => Action::Run(persist),
        }
    }
}

fn pixels(value: f32) -> String {
    format!("{} px", value.round())
}

fn percent(value: f32) -> String {
    format!("{} %", value.round())
}

fn disconnected_tooltip(layout: &PanelLayout) -> Tooltip {
    Tooltip::table(vec![row(
        "Groups",
        if layout.disconnected_groups {
            "separate frames"
        } else {
            "one bar"
        },
    )])
}

fn auto_collapse_tooltip(layout: &PanelLayout) -> Tooltip {
    Tooltip::table(vec![row(
        "Groups",
        if layout.auto_collapse_groups {
            "collapse when another window takes focus"
        } else {
            "stay open until the handle is clicked"
        },
    )])
}

fn workspaces_placement_tooltip(layout: &PanelLayout) -> Tooltip {
    Tooltip::table(vec![row(
        "Workspaces",
        match layout.workspaces_placement {
            WorkspacesPlacement::Sidebar => "in a sidebar on the left edge",
            WorkspacesPlacement::BottomPanel => "inline in the bottom panel",
        },
    )])
}

pub struct PanelSettingsGroup {
    pub(crate) expansion: GroupExpansion,
    sliders: [SliderControl; 5],
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Expansion(ExpansionMessage),
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    ToggleWorkspacesPlacement,
    ToggleShadow,
    Slider(Knob, ControlMessage),
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            sliders: Knob::ALL.map(Knob::control),
        }
    }
}

impl PanelSettingsGroup {
    fn slider(&self, knob: Knob) -> &SliderControl {
        &self.sliders[knob as usize]
    }

    pub fn tooltip_for(&self, target: &HoverTarget, layout: &PanelLayout) -> Option<Tooltip> {
        match target {
            HoverTarget::DisconnectedGroups => Some(disconnected_tooltip(layout)),
            HoverTarget::AutoCollapseGroups => Some(auto_collapse_tooltip(layout)),
            HoverTarget::WorkspacesPlacement => Some(workspaces_placement_tooltip(layout)),
            HoverTarget::DropShadow => Some(switch_tooltip(
                "Shadow",
                layout.shadow.cast,
                "on — panels cast a drop shadow",
            )),
            _ => Knob::ALL
                .into_iter()
                .find(|knob| knob.target() == *target)
                .map(|knob| {
                    Tooltip::table(vec![row(knob.label(), self.slider(knob).readout_text())])
                }),
        }
    }

    fn close_sliders(&mut self) {
        self.sliders.iter_mut().for_each(SliderControl::close);
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: PanelSettingsMessage, layout: &mut PanelLayout) -> Action {
        match msg {
            PanelSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Action::None
            }
            PanelSettingsMessage::ToggleDisconnectedGroups => {
                layout.disconnected_groups = !layout.disconnected_groups;
                Action::Run(
                    Task::future(state::panel::save_disconnected_groups(
                        layout.disconnected_groups,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleAutoCollapseGroups => {
                layout.auto_collapse_groups = !layout.auto_collapse_groups;
                Action::Run(
                    Task::future(state::panel::save_auto_collapse_groups(
                        layout.auto_collapse_groups,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleWorkspacesPlacement => {
                layout.workspaces_placement = layout.workspaces_placement.toggled();
                Action::Run(
                    Task::future(state::panel::save_workspaces_placement(
                        layout.workspaces_placement,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleShadow => {
                layout.shadow.cast = !layout.shadow.cast;
                Action::Run(
                    Task::future(state::panel::save_drop_shadow(layout.shadow.cast)).discard(),
                )
            }
            PanelSettingsMessage::Slider(knob, control) => {
                let SliderAction::Commit(value) = self.sliders[knob as usize].update(control)
                else {
                    return Action::None;
                };
                knob.committed(knob.write(layout, value).map(move |ok| {
                    Message::PanelSettings(PanelSettingsMessage::Slider(
                        knob,
                        ControlMessage::Persisted { value, ok },
                    ))
                }))
            }
        }
    }

    pub fn adopt(&mut self, layout: &PanelLayout) {
        for knob in Knob::ALL {
            self.sliders[knob as usize].update(ControlMessage::Reading(knob.read(layout)));
        }
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        layout: &PanelLayout,
    ) -> WidgetGroup<'a, PanelSettingsMessage> {
        let toggles = WidgetGroup::new(&self.expansion, PanelSettingsMessage::Expansion)
            .icon(Phosphor::SlidersHorizontal(Weight::Regular))
            .item(icon_toggle_item(
                config,
                Phosphor::Pill(Weight::Regular),
                layout.disconnected_groups,
                PanelSettingsMessage::ToggleDisconnectedGroups,
                HoverTarget::DisconnectedGroups,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::PushPin(Weight::Regular),
                !layout.auto_collapse_groups,
                PanelSettingsMessage::ToggleAutoCollapseGroups,
                HoverTarget::AutoCollapseGroups,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::SidebarSimple(Weight::Regular),
                layout.workspaces_placement == WorkspacesPlacement::Sidebar,
                PanelSettingsMessage::ToggleWorkspacesPlacement,
                HoverTarget::WorkspacesPlacement,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::SelectionBackground(Weight::Regular),
                layout.shadow.cast,
                PanelSettingsMessage::ToggleShadow,
                HoverTarget::DropShadow,
            ));
        Knob::ALL.into_iter().fold(toggles, |group, knob| {
            group.item(GroupItem::slider(
                self.slider(knob),
                config,
                move |control| PanelSettingsMessage::Slider(knob, control),
            ))
        })
    }
}
