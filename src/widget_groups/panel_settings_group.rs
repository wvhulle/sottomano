use std::{future::Future, ops::RangeInclusive};

use iced::Task;

use crate::{
    app::Message,
    config::Config,
    state::{self, ChromeState, WorkspacesPlacement},
    wayland_layers::tooltip::{HoverTarget, hoverable},
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::icon_toggle_glyph,
        tooltip_body::{Tooltip, row},
        widget_group::{ExpansionMessage, GroupEvent, GroupExpansion, WidgetGroup},
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
const RADIUS_RANGE: RangeInclusive<f32> = 0.0..=16.0;
const TINT_RANGE: RangeInclusive<f32> = 0.0..=100.0;

fn confirmed(
    value: f32,
    save: impl Future<Output = bool> + Send + 'static,
    wrap: fn(ControlMessage) -> PanelSettingsMessage,
) -> Task<Message> {
    Task::future(save)
        .map(move |ok| Message::PanelSettings(wrap(ControlMessage::Confirmed { value, ok })))
}

fn blur_tooltip(chrome: &ChromeState) -> Tooltip {
    Tooltip::table(vec![row(
        "Blur",
        if chrome.blur_background {
            "on — frosted behind the chrome"
        } else {
            "off"
        },
    )])
}

fn disconnected_tooltip(chrome: &ChromeState) -> Tooltip {
    Tooltip::table(vec![row(
        "Groups",
        if chrome.disconnected_groups {
            "separate pills"
        } else {
            "one bar"
        },
    )])
}

fn auto_collapse_tooltip(chrome: &ChromeState) -> Tooltip {
    Tooltip::table(vec![row(
        "Groups",
        if chrome.auto_collapse_groups {
            "collapse when another window takes focus"
        } else {
            "stay open until the handle is clicked"
        },
    )])
}

fn workspaces_placement_tooltip(chrome: &ChromeState) -> Tooltip {
    Tooltip::table(vec![row(
        "Workspaces",
        match chrome.workspaces_placement {
            WorkspacesPlacement::Sidebar => "in a sidebar on the left edge",
            WorkspacesPlacement::BottomPanel => "inline in the bottom panel",
        },
    )])
}

pub struct PanelSettingsGroup {
    pub(crate) expansion: GroupExpansion,
    radius: SliderControl,
    offset: SliderControl,
    tint: SliderControl,
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Expansion(ExpansionMessage),
    ToggleBlurBackground,
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    ToggleWorkspacesPlacement,
    Radius(ControlMessage),
    Offset(ControlMessage),
    Tint(ControlMessage),
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            radius: SliderControl::new(
                RADIUS_RANGE,
                HoverTarget::CornerRadius,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} px", value.round())),
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
    pub fn tooltip_for(&self, target: &HoverTarget, chrome: &ChromeState) -> Option<Tooltip> {
        match target {
            HoverTarget::CornerRadius => Some(Tooltip::table(vec![row(
                "Corners",
                format!("{} radius", self.radius.readout_text()),
            )])),
            HoverTarget::FloatOffset => Some(Tooltip::table(vec![row(
                "Offset",
                self.offset.readout_text(),
            )])),
            HoverTarget::TintChrome => Some(Tooltip::table(vec![row(
                "Tint",
                format!("{} tint over the chrome", self.tint.readout_text()),
            )])),
            HoverTarget::BlurBackground => Some(blur_tooltip(chrome)),
            HoverTarget::DisconnectedGroups => Some(disconnected_tooltip(chrome)),
            HoverTarget::AutoCollapseGroups => Some(auto_collapse_tooltip(chrome)),
            HoverTarget::WorkspacesPlacement => Some(workspaces_placement_tooltip(chrome)),
            _ => None,
        }
    }

    fn close_sliders(&mut self) {
        self.radius.close();
        self.offset.close();
        self.tint.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: PanelSettingsMessage, chrome: &mut ChromeState) -> Action {
        match msg {
            PanelSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Action::None
            }
            PanelSettingsMessage::ToggleBlurBackground => {
                chrome.blur_background = !chrome.blur_background;
                Action::RebuildBlur(
                    Task::future(state::panel::save_blur_background(chrome.blur_background))
                        .discard(),
                )
            }
            PanelSettingsMessage::ToggleDisconnectedGroups => {
                chrome.disconnected_groups = !chrome.disconnected_groups;
                Action::RebuildBlur(
                    Task::future(state::panel::save_disconnected_groups(
                        chrome.disconnected_groups,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleAutoCollapseGroups => {
                chrome.auto_collapse_groups = !chrome.auto_collapse_groups;
                Action::Run(
                    Task::future(state::panel::save_auto_collapse_groups(
                        chrome.auto_collapse_groups,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleWorkspacesPlacement => {
                chrome.workspaces_placement = chrome.workspaces_placement.toggled();
                Action::PlaceWorkspaces(
                    Task::future(state::panel::save_workspaces_placement(
                        chrome.workspaces_placement,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::Radius(msg) => match self.radius.update(msg) {
                SliderAction::Commit(radius) => {
                    chrome.corner_radius_px = radius;
                    Action::Run(confirmed(
                        radius,
                        state::panel::save_corner_radius(radius),
                        PanelSettingsMessage::Radius,
                    ))
                }
                _ => Action::None,
            },
            PanelSettingsMessage::Offset(msg) => match self.offset.update(msg) {
                SliderAction::Commit(offset) => {
                    chrome.float_offset_px = offset;
                    Action::Reflow(confirmed(
                        offset,
                        state::panel::save_float_offset(offset),
                        PanelSettingsMessage::Offset,
                    ))
                }
                _ => Action::None,
            },
            PanelSettingsMessage::Tint(msg) => match self.tint.update(msg) {
                SliderAction::Commit(percent) => {
                    chrome.chrome_tint_factor = percent / 100.0;
                    Action::Run(confirmed(
                        percent,
                        state::panel::save_chrome_tint_factor(chrome.chrome_tint_factor),
                        PanelSettingsMessage::Tint,
                    ))
                }
                _ => Action::None,
            },
        }
    }

    pub fn adopt(&mut self, chrome: &ChromeState) {
        self.radius
            .update(ControlMessage::Reading(chrome.corner_radius_px));
        self.offset
            .update(ControlMessage::Reading(chrome.float_offset_px));
        self.tint
            .update(ControlMessage::Reading(chrome.chrome_tint_factor * 100.0));
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        chrome: &ChromeState,
    ) -> WidgetGroup<'a, PanelSettingsMessage> {
        WidgetGroup::new(&self.expansion, PanelSettingsMessage::Expansion)
            .icon(Phosphor::SlidersHorizontal(Weight::Regular))
            .item(icon_toggle_item(
                config,
                Phosphor::Pill(Weight::Regular),
                chrome.disconnected_groups,
                PanelSettingsMessage::ToggleDisconnectedGroups,
                HoverTarget::DisconnectedGroups,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::PushPin(Weight::Regular),
                !chrome.auto_collapse_groups,
                PanelSettingsMessage::ToggleAutoCollapseGroups,
                HoverTarget::AutoCollapseGroups,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::SidebarSimple(Weight::Regular),
                chrome.workspaces_placement == WorkspacesPlacement::Sidebar,
                PanelSettingsMessage::ToggleWorkspacesPlacement,
                HoverTarget::WorkspacesPlacement,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::Snowflake(Weight::Regular),
                chrome.blur_background,
                PanelSettingsMessage::ToggleBlurBackground,
                HoverTarget::BlurBackground,
            ))
            .item(GroupItem::slider(
                &self.tint,
                config,
                PanelSettingsMessage::Tint,
            ))
            .item(GroupItem::slider(
                &self.radius,
                config,
                PanelSettingsMessage::Radius,
            ))
            .item(GroupItem::slider(
                &self.offset,
                config,
                PanelSettingsMessage::Offset,
            ))
    }
}

fn icon_toggle_item(
    config: &Config,
    icon: Phosphor,
    enabled: bool,
    press: PanelSettingsMessage,
    target: HoverTarget,
) -> GroupItem<'_, PanelSettingsMessage> {
    GroupItem::hoverable_element(move |_| {
        hoverable(
            icon_toggle_glyph(config, icon, enabled),
            target.clone(),
            GroupEvent::Tooltip,
        )
    })
    .on_press(press)
}
