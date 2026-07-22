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
    Alignment, Element,
    time::Instant,
    widget::{Row, mouse_area, slider},
};

use crate::{
    bar_program::Message,
    data_sources::row,
    layer_surface::Position,
    tooltip::hoverable,
    user_config::Config,
    widgets::{icon::Icon, menu_group::MenuGroup},
};

pub struct PanelSettingsGroup {
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone, Copy)]
pub enum PanelSettingsMessage {
    Toggle,
    Hover(bool),
    Close,
    ToggleFloating,
    SetFloatOffset(f32),
}

impl PanelSettingsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
        now: Instant,
    ) -> Option<Element<'a, Message>> {
        let foreground = config.palette.bar_foreground();
        let handle: Element<'a, Message> = Icon::Sliders
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
                    .on_press(Message::PanelSettings(PanelSettingsMessage::ToggleFloating))
                    .into(),
                vec![row(
                    "Floating",
                    if floating {
                        "on — rounded, inset from edges"
                    } else {
                        "off — flush, square"
                    },
                )],
            )
        };

        let offset = floating.then(|| {
            Knob {
                icon: Icon::ArrowsToCircle,
                range: 0.0..=24.0,
                step: 1.0,
            }
            .view(config, config.layout.float_offset_px, |v| {
                Message::PanelSettings(PanelSettingsMessage::SetFloatOffset(v))
            })
        });

        let reveal = self.menu.reveal(now);
        let content = Row::with_children([Some(floating_toggle), offset].into_iter().flatten())
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);
        let group = self
            .menu
            .view(config, position, reveal, handle, content.into());

        Some(
            mouse_area(group)
                .on_press(Message::PanelSettings(PanelSettingsMessage::Toggle))
                .on_enter(Message::PanelSettings(PanelSettingsMessage::Hover(true)))
                .on_exit(Message::PanelSettings(PanelSettingsMessage::Hover(false)))
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
        config: &Config,
        value: f32,
        on_change: impl Fn(f32) -> Message + 'a,
    ) -> Element<'a, Message> {
        let icon = self
            .icon
            .solid()
            .size(config.font.icon_size_px)
            .color(config.palette.bar_foreground());
        Row::with_children([
            icon.into(),
            slider(self.range, value, on_change)
                .step(self.step)
                .width(config.layout.slider_width_px)
                .into(),
        ])
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
    }
}
