//! The tray context-menu popup: an interactive layer surface the bar owns
//! (mirroring [`TooltipHost`](crate::tooltip::TooltipHost) but taking input),
//! whose body is rendered by
//! [`widgets::context_menu`](crate::widgets::context_menu). It lives here
//! rather than in `widgets/` because it emits
//! [`Message`](crate::bar_program::Message) and creates/destroys surfaces,
//! which the message-generic widgets never do. Unlike the tooltip, it carries
//! its content — a snapshot of the item's menu taken when it opens.

use std::collections::HashSet;

use iced::{
    Background, Border, Color, Element, Length, Rectangle, Size, Task,
    widget::{Sensor, container},
    window::Id,
};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    bar_layout::tray_group::TrayMessage,
    bar_program::Message,
    config::resolved::ResolvedConfig,
    layer_surface::Position,
    tooltip::{MenuAnchor, panel_left, tooltip_margin},
    widgets::context_menu::{MenuNode, context_menu},
};

/// A generous initial size for the menu surface; the panel lays itself out
/// (shrink-to-fit) and a [`Sensor`] reports the true size, which then shrinks
/// the surface — the same settle dance as the tooltip.
const MENU_PROVISIONAL: (u32, u32) = (420, 800);
/// Fixed content width, so row highlights span the full menu and submenu
/// chevrons right-align. Long labels wrap.
const MENU_WIDTH: f32 = 250.0;
const MENU_PADDING: f32 = 6.0;
/// Border alpha, scaled by the settle paint.
const BORDER_ALPHA: f32 = 0.25;

/// The single open tray context-menu popup surface, if any. Owned by the bar.
#[derive(Default)]
pub struct TrayMenuHost {
    shown: Option<OpenMenu>,
}

struct OpenMenu {
    surface: Id,
    address: String,
    menu_path: String,
    nodes: Vec<MenuNode>,
    /// dbusmenu ids of the submenus currently expanded in place.
    submenu_open: HashSet<i32>,
    anchor: Rectangle,
    output_width: Option<i32>,
    position: Position,
    size: Option<(u32, u32)>,
    settled: bool,
}

impl TrayMenuHost {
    pub fn is_open_for(&self, address: &str) -> bool {
        self.shown
            .as_ref()
            .is_some_and(|menu| menu.address == address)
    }

    /// The open menu's target `(address, menu_path)`, for dispatching a click.
    pub fn target(&self) -> Option<(String, String)> {
        self.shown
            .as_ref()
            .map(|menu| (menu.address.clone(), menu.menu_path.clone()))
    }

    pub fn open(
        &mut self,
        address: String,
        menu_path: String,
        nodes: Vec<MenuNode>,
        place: &MenuAnchor,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let close = self.close();
        let surface = Id::unique();
        let settings = menu_settings(place, config);
        self.shown = Some(OpenMenu {
            surface,
            address,
            menu_path,
            nodes,
            submenu_open: HashSet::new(),
            anchor: place.anchor,
            output_width: place.output_width,
            position: place.position,
            size: None,
            settled: false,
        });
        Task::batch([
            close,
            Task::done(Message::NewLayerShell {
                settings,
                id: surface,
            }),
        ])
    }

    pub fn close(&mut self) -> Task<Message> {
        match self.shown.take() {
            Some(menu) => Task::done(Message::RemoveWindow(menu.surface)),
            None => Task::none(),
        }
    }

    pub fn forget(&mut self, surface: Id) {
        if self
            .shown
            .as_ref()
            .is_some_and(|menu| menu.surface == surface)
        {
            self.shown = None;
        }
    }

    pub fn toggle_submenu(&mut self, id: i32) {
        if let Some(menu) = &mut self.shown
            && !menu.submenu_open.remove(&id)
        {
            menu.submenu_open.insert(id);
        }
    }

    /// Shrink the surface to the measured panel size and re-place its left
    /// margin against the true width (dedup so the settle doesn't loop) —
    /// the menu counterpart of the tooltip's resize.
    pub fn resize(&mut self, surface: Id, size: Size, config: &ResolvedConfig) -> Task<Message> {
        let Some(menu) = &mut self.shown else {
            return Task::none();
        };
        let requested = (size.width.ceil() as u32, size.height.ceil() as u32);
        if menu.surface != surface || menu.size == Some(requested) {
            return Task::none();
        }
        menu.size = Some(requested);
        menu.settled = true;
        let bar = config.layout.outer_height_px as i32;
        let left = panel_left(menu.anchor.x, menu.output_width, requested.0);
        let margin = tooltip_margin(menu.position, left, bar);
        Task::batch([
            Task::done(Message::MarginChange {
                id: surface,
                margin,
            }),
            Task::done(Message::SizeChange {
                id: surface,
                size: requested,
            }),
        ])
    }

    pub fn view<'a>(
        &self,
        surface: Id,
        config: &'a ResolvedConfig,
    ) -> Option<Element<'a, Message>> {
        let menu = self.shown.as_ref().filter(|menu| menu.surface == surface)?;
        Some(menu_surface(surface, menu, config))
    }
}

fn menu_settings(place: &MenuAnchor, config: &ResolvedConfig) -> NewLayerShellSettings {
    let bar = config.layout.outer_height_px as i32;
    let left = panel_left(place.anchor.x, place.output_width, MENU_PROVISIONAL.0);
    let anchor = match place.position {
        Position::Top => Anchor::Top | Anchor::Left,
        Position::Bottom => Anchor::Bottom | Anchor::Left,
    };
    NewLayerShellSettings {
        size: Some(MENU_PROVISIONAL),
        exclusive_zone: Some(0),
        anchor,
        margin: Some(tooltip_margin(place.position, left, bar)),
        layer: Layer::Overlay,
        // No keyboard focus (like the toasts): taking focus would itself fire a
        // niri focus-change and self-dismiss. Dismissal is by selection, re-click,
        // the bar backdrop, or a real focus change.
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(place.output.clone()),
        // Interactive, unlike the click-through tooltip.
        events_transparent: false,
        namespace: Some("vetrata-tray-menu".to_string()),
    }
}

fn menu_surface<'a>(
    surface: Id,
    menu: &OpenMenu,
    config: &'a ResolvedConfig,
) -> Element<'a, Message> {
    // Transparent on the pre-settle frame so a near-right-edge menu that re-places
    // on settle doesn't flash at the provisional offset (as the tooltip does).
    let paint = if menu.settled { 1.0 } else { 0.0 };
    let body = context_menu(
        config,
        &menu.nodes,
        &menu.submenu_open,
        paint,
        &|id| Message::Tray(TrayMessage::MenuItemClicked(id)),
        &|id| Message::Tray(TrayMessage::ToggleSubmenu(id)),
    );
    // A popup needs an opaque background: force the palette background's alpha to
    // `paint` (its own alpha is 0 for the blur-backed bar, which the compositor
    // does not blur behind this namespace).
    let background = Color {
        a: paint,
        ..config.palette.bar_background
    };
    let panel = container(body)
        .width(Length::Fixed(MENU_WIDTH))
        // Vertical padding only — a horizontal inset would keep the separators off
        // the panel edges; entries carry their own horizontal padding.
        .padding([MENU_PADDING, 0.0])
        .style(move |_| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: config
                    .palette
                    .bar_foreground
                    .scale_alpha(BORDER_ALPHA * paint),
                width: 1.0,
                // Always rounded — a popup, not the flush bar (chrome_radius is 0
                // when the bar is docked).
                radius: f32::from(config.layout.corner_radius_px).into(),
            },
            ..Default::default()
        });
    Sensor::new(panel)
        .on_resize(move |size| Message::Tray(TrayMessage::MenuResized { surface, size }))
        .into()
}
