use std::{future::Future, ops::RangeInclusive};

use iced::{
    Alignment, Element, Task,
    widget::{Row, button},
};

use crate::{
    app::Message,
    config::Config,
    state::{self, ChromeState},
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        phosphor::{self, Phosphor},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        tooltip_body::{Tooltip, row},
    },
};

pub enum Action {
    None,
    Run(Task<Message>),
    RebuildBlur(Task<Message>),
    Reflow(Task<Message>),
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

pub struct PanelSettingsGroup {
    pub(crate) menu: MenuGroup,
    radius: SliderControl,
    offset: SliderControl,
    tint: SliderControl,
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Menu(MenuMessage),
    ToggleBlurBackground,
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    Radius(ControlMessage),
    Offset(ControlMessage),
    Tint(ControlMessage),
    Tooltip(TooltipMessage),
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        Self {
            menu: MenuGroup::default(),
            radius: SliderControl::new(
                RADIUS_RANGE,
                HoverTarget::CornerRadius,
                phosphor::FrameCorners::Regular.into(),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} px", value.round())),
            offset: SliderControl::new(
                OFFSET_RANGE,
                HoverTarget::FloatOffset,
                phosphor::ArrowsIn::Regular.into(),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} px", value.round())),
            tint: SliderControl::new(
                TINT_RANGE,
                HoverTarget::TintChrome,
                phosphor::DropHalf::Regular.into(),
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
            _ => None,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.radius.is_animating() || self.offset.is_animating() || self.tint.is_animating()
    }

    fn close_sliders(&mut self) {
        self.radius.close();
        self.offset.close();
        self.tint.close();
    }

    pub fn close_menu(&mut self) {
        if self.menu.close() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: PanelSettingsMessage, chrome: &mut ChromeState) -> Action {
        match msg {
            PanelSettingsMessage::Menu(menu) => {
                if self.menu.update(menu) {
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
            PanelSettingsMessage::Tooltip(_) => Action::None,
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

    #[allow(clippy::unnecessary_wraps)]
    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        chrome: &'a ChromeState,
        position: Position,
    ) -> Option<Element<'a, PanelSettingsMessage>> {
        let foreground = config.palette.bar_foreground;
        let face =
            move || phosphor::SlidersHorizontal::Regular.view(config.font.icon_size_px, foreground);

        let content = move |_reveal: f32| {
            Row::with_children([
                icon_toggle(
                    config,
                    if chrome.disconnected_groups {
                        phosphor::Pill::Fill.into()
                    } else {
                        phosphor::Pill::Light.into()
                    },
                    PanelSettingsMessage::ToggleDisconnectedGroups,
                    HoverTarget::DisconnectedGroups,
                ),
                icon_toggle(
                    config,
                    if chrome.auto_collapse_groups {
                        phosphor::PushPin::Light.into()
                    } else {
                        phosphor::PushPin::Fill.into()
                    },
                    PanelSettingsMessage::ToggleAutoCollapseGroups,
                    HoverTarget::AutoCollapseGroups,
                ),
                icon_toggle(
                    config,
                    if chrome.blur_background {
                        phosphor::Snowflake::Fill.into()
                    } else {
                        phosphor::Snowflake::Light.into()
                    },
                    PanelSettingsMessage::ToggleBlurBackground,
                    HoverTarget::BlurBackground,
                ),
                slider_element(&self.tint, config, PanelSettingsMessage::Tint),
                slider_element(&self.radius, config, PanelSettingsMessage::Radius),
                slider_element(&self.offset, config, PanelSettingsMessage::Offset),
            ])
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center)
            .into()
        };
        Some(
            self.menu
                .slot(face, content, PanelSettingsMessage::Menu)
                .view(config, position),
        )
    }
}

fn slider_element<'a>(
    slider: &SliderControl,
    config: &Config,
    wrap: fn(ControlMessage) -> PanelSettingsMessage,
) -> Element<'a, PanelSettingsMessage> {
    slider.view(config, 1.0).map(move |m| match m {
        ControlMessage::Tooltip(t) => PanelSettingsMessage::Tooltip(t),
        m => wrap(m),
    })
}

fn icon_toggle<'a>(
    config: &Config,
    icon: Phosphor,
    press: PanelSettingsMessage,
    target: HoverTarget,
) -> Element<'a, PanelSettingsMessage> {
    let color = config.palette.bar_foreground;
    let widget = icon.view(config.font.icon_size_px, color);
    hoverable(
        button(widget)
            .padding(0)
            .style(flat_button(color))
            .on_press(press)
            .into(),
        target,
        PanelSettingsMessage::Tooltip,
    )
}
