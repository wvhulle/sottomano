//! odin's `PanelSettings`: session-only layout knobs. The sliders handle
//! unfolds a floating toggle plus (when floating) an expandable float-offset
//! slider — a [`SliderControl`] like the idle/audio pills. The config-mutating
//! side lives in the application, which owns the config and the open surfaces.

use std::ops::RangeInclusive;

use iced::{
    Alignment, Element,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, Tooltip, TooltipBody, row},
    layer_surface::Position,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::{
        icon::Icon,
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl, View},
    },
};

const OFFSET_RANGE: RangeInclusive<f32> = 0.0..=24.0;

pub fn floating_tooltip(config: &ResolvedConfig) -> Tooltip {
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

#[allow(clippy::cast_possible_truncation)]
pub fn offset_tooltip(config: &ResolvedConfig) -> Tooltip {
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
}

#[derive(Debug, Clone, Copy)]
pub enum PanelSettingsMessage {
    Menu(MenuMessage),
    ToggleFloating,
    Offset(ControlMessage),
    Tooltip(TooltipMessage),
}

impl PanelSettingsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            menu: MenuGroup::new(menu_expand_ms),
            offset: SliderControl::default(),
        }
    }

    pub fn update(&mut self, msg: PanelSettingsMessage) {
        match msg {
            // Collapsing the menu also closes the offset slider.
            PanelSettingsMessage::Menu(m) => {
                if self.menu.update(m) {
                    self.offset.close();
                }
            }
            PanelSettingsMessage::ToggleFloating
            | PanelSettingsMessage::Offset(_)
            | PanelSettingsMessage::Tooltip(_) => {}
        }
    }

    /// Drive the offset control against the app-owned config value; the app
    /// applies the returned [`Action`].
    pub fn update_offset(&mut self, msg: ControlMessage, reading: f32) -> Action {
        self.offset.update(msg, reading, &OFFSET_RANGE, 1.0)
    }

    pub fn sync_offset(&mut self, value: f32) {
        self.offset.sync(value);
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, PanelSettingsMessage>> {
        let foreground = config.palette.bar_foreground;
        // The handle carries the toggle press (only the handle toggles).
        let handle: Element<'a, PanelSettingsMessage> = mouse_area(
            Icon::Sliders
                .solid()
                .size(config.font.icon_size_px)
                .color(foreground),
        )
        .on_press(PanelSettingsMessage::Menu(MenuMessage::Toggle))
        .into();

        let floating = config.layout.floating;
        let floating_toggle = {
            let color = foreground.scale_alpha(if floating { 1.0 } else { 0.4 });
            let icon = Icon::Compress
                .solid()
                .size(config.font.icon_size_px)
                .color(color);
            hoverable(
                mouse_area(icon)
                    .on_press(PanelSettingsMessage::ToggleFloating)
                    .into(),
                TooltipSource::Floating,
                PanelSettingsMessage::Tooltip,
            )
        };

        let offset = floating.then(|| self.offset_element(config));

        let reveal = self.menu.reveal();
        let content = Row::with_children([Some(floating_toggle), offset].into_iter().flatten())
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

    #[allow(clippy::cast_possible_truncation)]
    fn offset_element<'a>(&self, config: &ResolvedConfig) -> Element<'a, PanelSettingsMessage> {
        let value = self.offset.value(config.float_offset_px);
        let widget = IconOnly {
            icon: Icon::ArrowsToCircle,
        };
        self.offset
            .view(
                config,
                View {
                    widget: &widget,
                    value,
                    readout: format!("{} px", value.round() as i64),
                    readout_width: 44.0,
                    range: OFFSET_RANGE,
                    step: 1.0,
                    source: TooltipSource::FloatOffset,
                    right_click: false,
                    alpha: 1.0,
                },
            )
            .map(|m| match m {
                ControlMessage::Tooltip(t) => PanelSettingsMessage::Tooltip(t),
                m => PanelSettingsMessage::Offset(m),
            })
    }
}

/// A bare icon as a `BarWidget` (no gauge or label) — the offset's closed
/// state.
struct IconOnly {
    icon: Icon,
}

impl BarWidget for IconOnly {
    fn icon(&self) -> Icon {
        self.icon
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}
