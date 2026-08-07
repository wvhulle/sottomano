use std::ops::RangeInclusive;

use iced::{
    Alignment, Element,
    widget::{Row, button},
};

use crate::{
    config::Config,
    data::source::{Tooltip, font_awesome::Solid, row},
    state::ChromeState,
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        DISABLED_ALPHA, flat_button,
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl},
    },
};

const OFFSET_RANGE: RangeInclusive<f32> = 0.0..=24.0;
const DARKEN_RANGE: RangeInclusive<f32> = 0.0..=100.0;

pub fn tooltip_for(target: &HoverTarget, chrome: &ChromeState) -> Option<Tooltip> {
    match target {
        HoverTarget::Floating => Some(floating_tooltip(chrome)),
        HoverTarget::FloatOffset => Some(offset_tooltip(chrome)),
        HoverTarget::BlurBackground => Some(blur_tooltip(chrome)),
        HoverTarget::DisconnectedGroups => Some(disconnected_tooltip(chrome)),
        HoverTarget::DarkenChrome => Some(darken_tooltip(chrome)),
        _ => None,
    }
}

fn floating_tooltip(chrome: &ChromeState) -> Tooltip {
    Tooltip::table(vec![row(
        "Floating",
        if chrome.floating {
            "on — rounded, inset from edges"
        } else {
            "off — flush, square"
        },
    )])
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

#[allow(clippy::cast_possible_truncation)]
fn darken_tooltip(chrome: &ChromeState) -> Tooltip {
    Tooltip::table(vec![row(
        "Darken",
        format!(
            "{} % black tint over the chrome",
            (chrome.darken_chrome_factor * 100.0).round() as i64
        ),
    )])
}

#[allow(clippy::cast_possible_truncation)]
fn offset_tooltip(chrome: &ChromeState) -> Tooltip {
    Tooltip::table(vec![row(
        "Offset",
        format!("{} px", chrome.float_offset_px.round() as i64),
    )])
}

pub struct PanelSettingsGroup {
    pub(crate) menu: MenuGroup,
    offset: SliderControl,
    darken: SliderControl,
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Menu(MenuMessage),
    ToggleFloating,
    ToggleBlurBackground,
    ToggleDisconnectedGroups,
    Offset(ControlMessage),
    Darken(ControlMessage),
    Tooltip(TooltipMessage),
}

impl CarriesTooltip for PanelSettingsMessage {
    fn into_tooltip(self) -> Result<TooltipMessage, Self> {
        match self {
            Self::Tooltip(tooltip) => Ok(tooltip),
            message => Err(message),
        }
    }

    fn from_tooltip(tooltip: TooltipMessage) -> Self {
        Self::Tooltip(tooltip)
    }
}

impl PanelSettingsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            menu: MenuGroup::new(menu_expand_ms),
            offset: SliderControl::new(
                OFFSET_RANGE,
                HoverTarget::FloatOffset,
                Solid::ArrowsToCircle.into(),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} px", value.round())),
            darken: SliderControl::new(DARKEN_RANGE, HoverTarget::DarkenChrome, Solid::XRay.into())
                .icon_only()
                .readout_width(44.0)
                .readout(|value| format!("{} %", value.round())),
        }
    }

    fn close_sliders(&mut self) {
        self.offset.close();
        self.darken.close();
    }

    pub fn close_menu(&mut self) {
        if self.menu.close() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: &PanelSettingsMessage) {
        match msg {
            PanelSettingsMessage::Menu(menu) => {
                if self.menu.update(*menu) {
                    self.close_sliders();
                }
            }
            PanelSettingsMessage::ToggleFloating
            | PanelSettingsMessage::ToggleBlurBackground
            | PanelSettingsMessage::ToggleDisconnectedGroups
            | PanelSettingsMessage::Offset(_)
            | PanelSettingsMessage::Darken(_)
            | PanelSettingsMessage::Tooltip(_) => {}
        }
    }

    pub fn update_offset(&mut self, msg: ControlMessage) -> Action {
        self.offset.update(msg)
    }

    pub fn sync_offset(&mut self, value: f32) {
        self.offset.update(ControlMessage::Reading(value));
    }

    pub fn update_darken(&mut self, msg: ControlMessage) -> Action {
        self.darken.update(msg)
    }

    pub fn sync_darken(&mut self, value: f32) {
        self.darken.update(ControlMessage::Reading(value));
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn view<'a>(
        &self,
        config: &Config,
        chrome: &ChromeState,
        position: Position,
    ) -> Option<Element<'a, PanelSettingsMessage>> {
        let foreground = config.palette.bar_foreground;
        let handle: Element<'a, PanelSettingsMessage> = button(
            Solid::Sliders
                .widget()
                .size(config.font.icon_size_px)
                .color(foreground),
        )
        .padding(0)
        .style(flat_button(foreground))
        .on_press(PanelSettingsMessage::Menu(MenuMessage::Toggle))
        .into();

        let floating = chrome.floating;
        let disconnected_toggle = icon_toggle(
            config,
            Solid::Capsules,
            chrome.disconnected_groups,
            PanelSettingsMessage::ToggleDisconnectedGroups,
            HoverTarget::DisconnectedGroups,
        );
        let floating_toggle = icon_toggle(
            config,
            Solid::Compress,
            floating,
            PanelSettingsMessage::ToggleFloating,
            HoverTarget::Floating,
        );
        let blur_toggle = icon_toggle(
            config,
            Solid::Snowflake,
            chrome.blur_background,
            PanelSettingsMessage::ToggleBlurBackground,
            HoverTarget::BlurBackground,
        );

        let offset = floating.then(|| self.offset_element(config));
        let darken = self.darken_element(config);

        let reveal = self.menu.reveal();
        let content = Row::with_children(
            [
                Some(disconnected_toggle),
                Some(floating_toggle),
                Some(blur_toggle),
                Some(darken),
                offset,
            ]
            .into_iter()
            .flatten(),
        )
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Alignment::Center);
        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            PanelSettingsMessage::Menu,
        ))
    }

    fn offset_element<'a>(&self, config: &Config) -> Element<'a, PanelSettingsMessage> {
        self.offset
            .view(config, 1.0)
            .map(|m| m.lifted(PanelSettingsMessage::Offset))
    }

    fn darken_element<'a>(&self, config: &Config) -> Element<'a, PanelSettingsMessage> {
        self.darken
            .view(config, 1.0)
            .map(|m| m.lifted(PanelSettingsMessage::Darken))
    }
}

fn icon_toggle<'a>(
    config: &Config,
    icon: Solid,
    active: bool,
    press: PanelSettingsMessage,
    target: HoverTarget,
) -> Element<'a, PanelSettingsMessage> {
    let alpha = if active { 1.0 } else { DISABLED_ALPHA };
    let color = config.palette.bar_foreground.scale_alpha(alpha);
    let widget = icon.widget().size(config.font.icon_size_px).color(color);
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
