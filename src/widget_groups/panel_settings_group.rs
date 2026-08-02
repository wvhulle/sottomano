use std::ops::RangeInclusive;

use iced::{
    Alignment, Element,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{Tooltip, TooltipBody, font_awesome::Solid, row},
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl},
    },
};

const OFFSET_RANGE: RangeInclusive<f32> = 0.0..=24.0;
const DARKEN_RANGE: RangeInclusive<f32> = 0.0..=100.0;

pub fn tooltip_for(target: &HoverTarget, config: &ResolvedConfig) -> Option<Tooltip> {
    match target {
        HoverTarget::Floating => Some(floating_tooltip(config)),
        HoverTarget::FloatOffset => Some(offset_tooltip(config)),
        HoverTarget::BlurBackground => Some(blur_tooltip(config)),
        HoverTarget::DisconnectedGroups => Some(disconnected_tooltip(config)),
        HoverTarget::DarkenChrome => Some(darken_tooltip(config)),
        _ => None,
    }
}

fn floating_tooltip(config: &ResolvedConfig) -> Tooltip {
    Tooltip {
        body: TooltipBody::Table(vec![row(
            "Floating",
            if config.layout.floating {
                "on — rounded, inset from edges"
            } else {
                "off — flush, square"
            },
        )]),
        footer: None,
    }
}

fn blur_tooltip(config: &ResolvedConfig) -> Tooltip {
    Tooltip {
        body: TooltipBody::Table(vec![row(
            "Blur",
            if config.layout.blur_background {
                "on — frosted behind the chrome"
            } else {
                "off"
            },
        )]),
        footer: None,
    }
}

fn disconnected_tooltip(config: &ResolvedConfig) -> Tooltip {
    Tooltip {
        body: TooltipBody::Table(vec![row(
            "Groups",
            if config.layout.disconnected_groups {
                "separate pills"
            } else {
                "one bar"
            },
        )]),
        footer: None,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn darken_tooltip(config: &ResolvedConfig) -> Tooltip {
    Tooltip {
        body: TooltipBody::Table(vec![row(
            "Darken",
            format!(
                "{} % black tint over the chrome",
                (config.darken_chrome_factor * 100.0).round() as i64
            ),
        )]),
        footer: None,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn offset_tooltip(config: &ResolvedConfig) -> Tooltip {
    Tooltip {
        body: TooltipBody::Table(vec![row(
            "Offset",
            format!("{} px", config.float_offset_px.round() as i64),
        )]),
        footer: None,
    }
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

    pub fn update(&mut self, msg: &PanelSettingsMessage) {
        match msg {
            PanelSettingsMessage::Menu(menu) => {
                if self.menu.update(*menu) {
                    self.offset.close();
                    self.darken.close();
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
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, PanelSettingsMessage>> {
        let foreground = config.palette.bar_foreground;
        let handle: Element<'a, PanelSettingsMessage> = mouse_area(
            Solid::Sliders
                .widget()
                .size(config.font.icon_size_px)
                .color(foreground),
        )
        .on_press(PanelSettingsMessage::Menu(MenuMessage::Toggle))
        .into();

        let floating = config.layout.floating;
        let disconnected_toggle = icon_toggle(
            config,
            Solid::Capsules,
            config.layout.disconnected_groups,
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
            config.layout.blur_background,
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
        .spacing(config.layout.horizontal_widget_spacing_px)
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

    fn offset_element<'a>(&self, config: &ResolvedConfig) -> Element<'a, PanelSettingsMessage> {
        self.offset.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => PanelSettingsMessage::Tooltip(t),
            m => PanelSettingsMessage::Offset(m),
        })
    }

    fn darken_element<'a>(&self, config: &ResolvedConfig) -> Element<'a, PanelSettingsMessage> {
        self.darken.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => PanelSettingsMessage::Tooltip(t),
            m => PanelSettingsMessage::Darken(m),
        })
    }
}

fn icon_toggle<'a>(
    config: &ResolvedConfig,
    icon: Solid,
    active: bool,
    press: PanelSettingsMessage,
    target: HoverTarget,
) -> Element<'a, PanelSettingsMessage> {
    let color = config
        .palette
        .bar_foreground
        .scale_alpha(if active { 1.0 } else { 0.4 });
    let widget = icon.widget().size(config.font.icon_size_px).color(color);
    hoverable(
        mouse_area(widget).on_press(press).into(),
        target,
        PanelSettingsMessage::Tooltip,
    )
}
