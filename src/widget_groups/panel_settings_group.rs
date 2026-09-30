use std::{future::Future, ops::RangeInclusive};

use iced::Task;

use crate::{
    app::Message,
    config::Config,
    state::{self, PanelLayout, WorkspacesPlacement},
    wayland_layers::tooltip::HoverTarget,
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
    RebuildBlur(Task<Message>),
    Reflow(Task<Message>),
    PlaceWorkspaces(Task<Message>),
}

const OFFSET_RANGE: RangeInclusive<f32> = 0.0..=24.0;
const TINT_RANGE: RangeInclusive<f32> = 0.0..=100.0;

fn persisted(
    value: f32,
    save: impl Future<Output = bool> + Send + 'static,
    wrap: fn(ControlMessage) -> PanelSettingsMessage,
) -> Task<Message> {
    Task::future(save)
        .map(move |ok| Message::PanelSettings(wrap(ControlMessage::Persisted { value, ok })))
}

fn disconnected_tooltip(layout: &PanelLayout) -> Tooltip {
    Tooltip::table(vec![row(
        "Groups",
        if layout.disconnected_groups {
            "separate pills"
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
    offset: SliderControl,
    tint: SliderControl,
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Expansion(ExpansionMessage),
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    ToggleWorkspacesPlacement,
    Offset(ControlMessage),
    Tint(ControlMessage),
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            offset: SliderControl::new(
                OFFSET_RANGE,
                HoverTarget::FloatOffset,
                Phosphor::ArrowsIn(Weight::Regular),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} px", value.round())),
            tint: SliderControl::new(
                TINT_RANGE,
                HoverTarget::TintChrome,
                Phosphor::DropHalf(Weight::Regular),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} %", value.round())),
        }
    }
}

impl PanelSettingsGroup {
    pub fn tooltip_for(&self, target: &HoverTarget, layout: &PanelLayout) -> Option<Tooltip> {
        match target {
            HoverTarget::FloatOffset => Some(Tooltip::table(vec![row(
                "Offset",
                self.offset.readout_text(),
            )])),
            HoverTarget::TintChrome => Some(Tooltip::table(vec![row(
                "Tint",
                format!("{} tint over the chrome", self.tint.readout_text()),
            )])),
            HoverTarget::DisconnectedGroups => Some(disconnected_tooltip(layout)),
            HoverTarget::AutoCollapseGroups => Some(auto_collapse_tooltip(layout)),
            HoverTarget::WorkspacesPlacement => Some(workspaces_placement_tooltip(layout)),
            _ => None,
        }
    }

    fn close_sliders(&mut self) {
        self.offset.close();
        self.tint.close();
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
                Action::RebuildBlur(
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
                Action::PlaceWorkspaces(
                    Task::future(state::panel::save_workspaces_placement(
                        layout.workspaces_placement,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::Offset(msg) => match self.offset.update(msg) {
                SliderAction::Commit(offset) => {
                    layout.float_offset_px = offset;
                    Action::Reflow(persisted(
                        offset,
                        state::panel::save_float_offset(offset),
                        PanelSettingsMessage::Offset,
                    ))
                }
                _ => Action::None,
            },
            PanelSettingsMessage::Tint(msg) => match self.tint.update(msg) {
                SliderAction::Commit(percent) => {
                    layout.tint_factor = percent / 100.0;
                    Action::Run(persisted(
                        percent,
                        state::panel::save_tint_factor(layout.tint_factor),
                        PanelSettingsMessage::Tint,
                    ))
                }
                _ => Action::None,
            },
        }
    }

    pub fn adopt(&mut self, layout: &PanelLayout) {
        self.offset
            .update(ControlMessage::Reading(layout.float_offset_px));
        self.tint
            .update(ControlMessage::Reading(layout.tint_factor * 100.0));
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        layout: &PanelLayout,
    ) -> WidgetGroup<'a, PanelSettingsMessage> {
        WidgetGroup::new(&self.expansion, PanelSettingsMessage::Expansion)
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
            .item(GroupItem::slider(
                &self.tint,
                config,
                PanelSettingsMessage::Tint,
            ))
            .item(GroupItem::slider(
                &self.offset,
                config,
                PanelSettingsMessage::Offset,
            ))
    }
}
