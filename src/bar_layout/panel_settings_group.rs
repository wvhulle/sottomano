//! odin's `PanelSettings`: live knobs for the panel's own layout, session-only
//! (an in-memory layout-config mutation, no `.toml` write-back). The handle
//! is a sliders glyph; toggling it unfolds a floating toggle plus (when
//! floating) an offset slider to its right.
//!
//! Odin's `merged`/`blur`/`shadow` toggles are omitted here: nothing in
//! vetrata's renderer currently reads those config fields (no per-group pill
//! chrome, no compositor blur region, no shadow backdrop caster — see
//! `TODO.md`), so a toggle for them would have no visible effect. Odin's
//! click-to-expand `SettingKnob` is also simplified to a directly draggable
//! slider, shown once already inside the revealed menu-group content.

use iced::{
    Alignment, Element,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::{Tooltip, TooltipBody, row},
    layer_surface::Position,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::{icon::Icon, knob::Knob, menu_group::MenuGroup},
};

/// The floating-toggle tooltip, derived from the current layout config. A free
/// function so the application's tooltip resolver can build it without the
/// group's view state. No footer — the toggle is config-derived, not polled.
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
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, PanelSettingsMessage>> {
        let foreground = config.palette.bar_foreground;
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
