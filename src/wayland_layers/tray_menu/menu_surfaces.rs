use std::collections::HashSet;

use iced::{Element, Task, window::Id};
use iced_layershell::reexport::{Anchor, Layer, NewLayerShellSettings};

use crate::{
    app::Message,
    config::Config,
    wayland_layers::{
        surface::{self, Margin, Position},
        tray_menu::{MenuAnchor, menu_content::menu_content},
    },
    widgets::context_menu::MenuNode,
};

#[derive(Default)]
pub struct TrayMenuHost {
    shown: Option<OpenMenu>,
}

pub struct OpenMenu {
    pub surface: Id,
    pub address: String,
    pub menu_path: String,
    pub nodes: Vec<MenuNode>,
    pub submenu_open: HashSet<i32>,
    pub anchor_x: f32,
    pub position: Position,
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
    ) -> Task<Message> {
        let close = self.close();
        let surface = Id::unique();
        self.shown = Some(OpenMenu {
            surface,
            address,
            menu_path,
            nodes,
            submenu_open: HashSet::new(),
            anchor_x: place.anchor.x,
            position: place.position,
        });
        Task::batch([
            close,
            Task::done(Message::NewLayerShell {
                settings: menu_settings(place),
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

    pub fn view<'a>(&self, surface: Id, config: &'a Config) -> Option<Element<'a, Message>> {
        let menu = self.shown.as_ref().filter(|menu| menu.surface == surface)?;
        Some(menu_content(menu, config))
    }
}

fn menu_settings(place: &MenuAnchor) -> NewLayerShellSettings {
    let margin = Margin {
        top: 0,
        right: place.float_offset,
        bottom: 0,
        left: place.float_offset,
    };
    NewLayerShellSettings {
        size: Some((0, 0)),
        exclusive_zone: Some(0),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        margin: Some(margin.edges()),
        layer: Layer::Overlay,
        ..surface::base_settings(Some(&place.output), "vetrata-tray-menu")
    }
}
