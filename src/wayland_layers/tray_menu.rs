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
    app::Message,
    config::resolved::ResolvedConfig,
    wayland_layers::{
        surface::{Position, SurfaceSize},
        tooltip::{MenuAnchor, panel_left, tooltip_margin},
    },
    widget_groups::tray_group::TrayMessage,
    widgets::context_menu::{MenuNode, context_menu},
};

const MENU_PROVISIONAL: SurfaceSize = SurfaceSize {
    width: 420,
    height: 800,
};
const MENU_WIDTH: f32 = 250.0;
const MENU_PADDING: f32 = 6.0;
const BORDER_ALPHA: f32 = 0.25;

#[derive(Default)]
pub struct TrayMenuHost {
    shown: Option<OpenMenu>,
}

struct OpenMenu {
    surface: Id,
    address: String,
    menu_path: String,
    nodes: Vec<MenuNode>,
    submenu_open: HashSet<i32>,
    anchor: Rectangle,
    output_width: Option<i32>,
    position: Position,
    size: Option<SurfaceSize>,
    settled: bool,
}

impl TrayMenuHost {
    pub fn is_open_for(&self, address: &str) -> bool {
        self.shown
            .as_ref()
            .is_some_and(|menu| menu.address == address)
    }

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

    pub fn resize(&mut self, surface: Id, size: Size, config: &ResolvedConfig) -> Task<Message> {
        let Some(menu) = &mut self.shown else {
            return Task::none();
        };
        let requested = SurfaceSize {
            width: size.width.ceil() as u32,
            height: size.height.ceil() as u32,
        };
        if menu.surface != surface || menu.size == Some(requested) {
            return Task::none();
        }
        menu.size = Some(requested);
        menu.settled = true;
        let bar = config.layout.outer_height_px as i32;
        let left = panel_left(menu.anchor.x, menu.output_width, requested.width);
        let margin = tooltip_margin(menu.position, left, bar);
        Task::batch([
            Task::done(Message::MarginChange {
                id: surface,
                margin: margin.edges(),
            }),
            Task::done(Message::SizeChange {
                id: surface,
                size: requested.dimensions(),
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
    let left = panel_left(place.anchor.x, place.output_width, MENU_PROVISIONAL.width);
    let anchor = match place.position {
        Position::Top => Anchor::Top | Anchor::Left,
        Position::Bottom => Anchor::Bottom | Anchor::Left,
    };
    NewLayerShellSettings {
        size: Some(MENU_PROVISIONAL.dimensions()),
        exclusive_zone: Some(0),
        anchor,
        margin: Some(tooltip_margin(place.position, left, bar).edges()),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(place.output.clone()),
        events_transparent: false,
        namespace: Some("vetrata-tray-menu".to_string()),
    }
}

fn menu_surface<'a>(
    surface: Id,
    menu: &OpenMenu,
    config: &'a ResolvedConfig,
) -> Element<'a, Message> {
    let paint = if menu.settled { 1.0 } else { 0.0 };
    let body = context_menu(
        config,
        &menu.nodes,
        &menu.submenu_open,
        paint,
        &|id| Message::Tray(TrayMessage::MenuItemClicked(id)),
        &|id| Message::Tray(TrayMessage::ToggleSubmenu(id)),
    );
    let background = Color {
        a: paint,
        ..config.palette.bar_background
    };
    let panel = container(body)
        .width(Length::Fixed(MENU_WIDTH))
        .padding([MENU_PADDING, 0.0])
        .style(move |_| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: config
                    .palette
                    .bar_foreground
                    .scale_alpha(BORDER_ALPHA * paint),
                width: 1.0,
                radius: f32::from(config.layout.corner_radius_px).into(),
            },
            ..Default::default()
        });
    Sensor::new(panel)
        .on_resize(move |size| Message::Tray(TrayMessage::MenuResized { surface, size }))
        .into()
}
