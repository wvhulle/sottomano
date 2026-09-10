use std::{collections::HashMap, path::Path, sync::Arc};

use iced::{
    Alignment, Element, Task,
    widget::{Row, button, hover, image, svg},
};

use crate::{
    config::Config,
    data::source::{app_icon::DesktopEntries, desktop_entries::DesktopIndex, niri},
    message_subscriptions::blocking,
    wayland_layers::surface::Position,
    widgets::{
        flat_button,
        font_awesome::Solid,
        menu_group::{MenuGroup, MenuMessage},
    },
};

pub const MAX_ITEMS: usize = 4;
const RESTING_OPACITY: f32 = 0.65;

#[derive(Default)]
pub struct QuickLaunch {
    ranked: HashMap<String, Vec<String>>,
    desktop_entries: Option<Arc<DesktopEntries>>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum QuickLaunchMessage {
    Menu(MenuMessage),
    Launch(String),
}

impl QuickLaunch {
    pub fn set_desktop_entries(&mut self, entries: Arc<DesktopEntries>) {
        self.desktop_entries = Some(entries);
    }

    pub fn set_ranked(&mut self, ranked: HashMap<String, Vec<String>>) {
        self.ranked = ranked;
    }

    pub fn icon_keys(&self) -> Vec<String> {
        self.ranked.values().flatten().cloned().collect()
    }

    pub fn update(&mut self, msg: QuickLaunchMessage) -> Task<QuickLaunchMessage> {
        match msg {
            QuickLaunchMessage::Menu(m) => {
                self.menu.update(m);
            }
            QuickLaunchMessage::Launch(app_id) => return self.launch(app_id),
        }
        Task::none()
    }

    fn launch(&self, app_id: String) -> Task<QuickLaunchMessage> {
        let command = self
            .desktop_entries
            .as_ref()
            .and_then(|entries| entries.exec_command(&app_id))
            .unwrap_or_else(|| vec![app_id]);
        blocking("quicklaunch launch", move || {
            niri::send_action(&niri_ipc::Action::Spawn { command });
        })
        .discard()
    }

    pub fn view<'a>(
        &'a self,
        index: &'a DesktopIndex,
        output: &str,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, QuickLaunchMessage>> {
        let app_ids = self.ranked.get(output).filter(|ids| !ids.is_empty())?;

        let handle = move || {
            button(
                Solid::Rocket
                    .glyph()
                    .size(config.font.icon_size_px)
                    .color(config.palette.bar_foreground),
            )
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(QuickLaunchMessage::Menu(MenuMessage::Toggle))
            .into()
        };

        let content = move |reveal: f32| {
            Row::with_children(
                app_ids
                    .iter()
                    .filter_map(|app_id| icon_slot(config, index, app_id, reveal)),
            )
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center)
            .into()
        };

        Some(
            self.menu
                .view(config, position, handle, content, QuickLaunchMessage::Menu),
        )
    }
}

fn icon_slot<'a>(
    config: &Config,
    index: &DesktopIndex,
    app_id: &str,
    reveal: f32,
) -> Option<Element<'a, QuickLaunchMessage>> {
    let path = index.app_icon(app_id)?;
    let side = config.font.icon_size_px;
    let resting = icon_image(path, side, reveal * RESTING_OPACITY);
    let bright = icon_image(path, side, reveal);
    Some(
        button(hover(resting, bright))
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(QuickLaunchMessage::Launch(app_id.to_string()))
            .into(),
    )
}

fn icon_image<'a>(path: &Path, side: f32, opacity: f32) -> Element<'a, QuickLaunchMessage> {
    let is_svg = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));
    if is_svg {
        svg(svg::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(opacity)
            .into()
    } else {
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(opacity)
            .into()
    }
}
