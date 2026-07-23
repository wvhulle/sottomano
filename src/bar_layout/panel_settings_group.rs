//! odin's `PanelSettings`: live knobs for the panel's own layout, session-only
//! (an in-memory layout-config mutation, no `.toml` write-back). The handle
//! is a sliders glyph; toggling it unfolds a floating toggle plus (when
//! floating) an offset slider to its right.
//!
//! Odin's `merged`/`blur`/`shadow` toggles are omitted here: nothing in
//! odin-rs's renderer currently reads those config fields (no per-group pill
//! chrome, no compositor blur region, no shadow backdrop caster — see
//! `TODO.md`), so a toggle for them would have no visible effect. Odin's
//! click-to-expand `SettingKnob` is also simplified to a directly draggable
//! slider, shown once already inside the revealed menu-group content.

use std::ops::RangeInclusive;

use iced::{
    Alignment, Border, Color, Element,
    widget::{Row, mouse_area, slider},
};

use crate::{
    data_sources::{Tooltip, TooltipBody, row},
    layer_surface::Position,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    user_config::RuntimeConfig,
    widgets::{icon::Icon, menu_group::MenuGroup},
};

/// The floating-toggle tooltip, derived from the current layout config. A free
/// function so the application's tooltip resolver can build it without the
/// group's view state. No footer — the toggle is config-derived, not polled.
pub fn floating_tooltip(config: &RuntimeConfig) -> Tooltip {
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

pub struct PanelSettingsGroup {
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Toggle,
    Hover(bool),
    Close,
    ToggleFloating,
    SetFloatOffset(f32),
    Tooltip(TooltipMessage),
}

impl PanelSettingsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    /// Menu-only state transitions; the config-mutating variants are handled
    /// by the application, which owns the config and the open surfaces.
    pub fn update(&mut self, msg: PanelSettingsMessage) {
        match msg {
            PanelSettingsMessage::Toggle => self.menu.toggle(),
            PanelSettingsMessage::Hover(hovered) => self.menu.set_hover(hovered),
            PanelSettingsMessage::Close => self.menu.close(),
            PanelSettingsMessage::ToggleFloating
            | PanelSettingsMessage::SetFloatOffset(_)
            | PanelSettingsMessage::Tooltip(_) => {}
        }
    }

    pub fn view<'a>(
        &self,
        config: &RuntimeConfig,
        position: Position,
    ) -> Option<Element<'a, PanelSettingsMessage>> {
        let foreground = config.palette.bar_foreground();
        let handle: Element<'a, PanelSettingsMessage> = Icon::Sliders
            .solid()
            .size(config.font.icon_size_px)
            .color(foreground)
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

        let offset = floating.then(|| {
            Knob {
                icon: Icon::ArrowsToCircle,
                range: 0.0..=24.0,
                step: 1.0,
            }
            .view(
                config,
                config.layout.float_offset_px,
                PanelSettingsMessage::SetFloatOffset,
            )
        });

        let reveal = self.menu.reveal();
        let content = Row::with_children([Some(floating_toggle), offset].into_iter().flatten())
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);
        let group = self
            .menu
            .view(config, position, reveal, handle, content.into());

        Some(
            mouse_area(group)
                .on_press(PanelSettingsMessage::Toggle)
                .on_enter(PanelSettingsMessage::Hover(true))
                .on_exit(PanelSettingsMessage::Hover(false))
                .into(),
        )
    }
}

struct Knob {
    icon: Icon,
    range: RangeInclusive<f32>,
    step: f32,
}

impl Knob {
    fn view<'a>(
        self,
        config: &RuntimeConfig,
        value: f32,
        on_change: impl Fn(f32) -> PanelSettingsMessage + 'a,
    ) -> Element<'a, PanelSettingsMessage> {
        let icon = self
            .icon
            .solid()
            .size(config.font.icon_size_px)
            .color(config.palette.bar_foreground());
        // Fill with the palette's `focus` accent rather than iced's default
        // bright-blue primary, which clashes with the bar's gruvbox tones.
        let accent = config.palette.focus();
        let track = config.palette.bar_foreground().scale_alpha(0.25);
        Row::with_children([
            icon.into(),
            slider(self.range, value, on_change)
                .step(self.step)
                .width(config.layout.slider_width_px)
                .style(move |_theme, status| {
                    let fill = match status {
                        slider::Status::Dragged => accent.scale_alpha(0.8),
                        _ => accent,
                    };
                    slider::Style {
                        rail: slider::Rail {
                            backgrounds: (fill.into(), track.into()),
                            width: 4.0,
                            border: Border {
                                radius: 2.0.into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                        },
                        handle: slider::Handle {
                            shape: slider::HandleShape::Circle { radius: 7.0 },
                            background: fill.into(),
                            border_color: Color::TRANSPARENT,
                            border_width: 0.0,
                        },
                    }
                })
                .into(),
        ])
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
    }
}
