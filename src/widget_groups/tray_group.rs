use iced::{
    Alignment, Element, Task,
    widget::{Column, Row, button, mouse_area},
    window::Id,
};

use crate::{
    app::Message,
    config::{Config, tray::ClickAction},
    data::source::{
        Tooltip, TooltipBody,
        font_awesome::Solid,
        tray::{self, Status, TrayCommand, TrayHandle, TrayItem, TrayUpdate},
    },
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
        tray_menu::{MenuAnchor, TrayMenuHost},
    },
    widgets::{
        activity_line::activity_line,
        context_menu::MenuNode,
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        native_icon::native_icon,
    },
};

#[derive(Default)]
pub struct TrayGroup {
    items: Vec<TrayItem>,
    handle: Option<TrayHandle>,
    menu_host: TrayMenuHost,
    pub(crate) menu: MenuGroup,
}

struct MenuTarget {
    address: String,
    menu_path: String,
    nodes: Vec<MenuNode>,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Update(TrayUpdate),
    Menu(MenuMessage),
    Activate(String),
    Secondary(String),
    OpenMenu(usize),
    FocusWindow {
        window_app_id: String,
        item_id: String,
    },
    MenuItemClicked(i32),
    ToggleSubmenu(i32),
    Tooltip(TooltipMessage),
}

pub enum Action {
    None,
    Run(Task<Message>),
    MenuOpened(Task<Message>),
    FocusWindow {
        window_app_id: String,
        item_id: String,
    },
    Tooltip(TooltipMessage),
}

impl TrayGroup {
    pub fn subscription() -> iced::Subscription<TrayMessage> {
        tray::subscription().map(TrayMessage::Update)
    }

    pub fn update(
        &mut self,
        message: TrayMessage,
        config: &Config,
        anchor: Option<MenuAnchor>,
    ) -> Action {
        match message {
            TrayMessage::Update(TrayUpdate::Connected(handle)) => {
                self.handle = Some(handle);
                Action::None
            }
            TrayMessage::Update(TrayUpdate::Items(items)) => {
                self.items = items;
                Action::None
            }
            TrayMessage::Menu(m) => {
                self.menu.update(m);
                Action::None
            }
            TrayMessage::Activate(address) => {
                self.command(TrayCommand::Activate(address));
                Action::None
            }
            TrayMessage::Secondary(address) => {
                self.command(TrayCommand::Secondary(address));
                Action::None
            }
            TrayMessage::FocusWindow {
                window_app_id,
                item_id,
            } => Action::FocusWindow {
                window_app_id,
                item_id,
            },
            TrayMessage::OpenMenu(index) => self.open_menu(index, config, anchor),
            TrayMessage::MenuItemClicked(id) => {
                if let Some((address, menu_path)) = self.menu_host.target() {
                    self.command(TrayCommand::MenuClick {
                        address,
                        menu_path,
                        id,
                    });
                }
                Action::Run(self.menu_host.close())
            }
            TrayMessage::ToggleSubmenu(id) => {
                self.menu_host.toggle_submenu(id);
                Action::None
            }
            TrayMessage::Tooltip(message) => Action::Tooltip(message),
        }
    }

    fn open_menu(&mut self, index: usize, config: &Config, anchor: Option<MenuAnchor>) -> Action {
        let Some(target) = self.menu_target(index, config) else {
            return Action::None;
        };
        if self.menu_host.is_open_for(&target.address) {
            return Action::Run(self.menu_host.close());
        }
        let Some(place) = anchor else {
            return Action::None;
        };
        Action::MenuOpened(self.menu_host.open(
            target.address,
            target.menu_path,
            target.nodes,
            &place,
        ))
    }

    fn menu_target(&self, index: usize, config: &Config) -> Option<MenuTarget> {
        let visible = self.visible(config);
        let item = visible.get(index)?;
        let Some(menu_path) = item.menu_path.as_ref().filter(|_| !item.menu.is_empty()) else {
            eprintln!("tray: {}: no menu", item.id);
            return None;
        };
        Some(MenuTarget {
            address: item.address.clone(),
            menu_path: menu_path.clone(),
            nodes: item.menu.clone(),
        })
    }

    pub fn close_menu(&mut self) -> Task<Message> {
        self.menu_host.close()
    }

    pub fn forget(&mut self, surface: Id) {
        self.menu_host.forget(surface);
    }

    pub fn menu_view<'a>(&self, surface: Id, config: &'a Config) -> Option<Element<'a, Message>> {
        self.menu_host.view(surface, config)
    }

    fn command(&self, command: TrayCommand) {
        if let Some(handle) = &self.handle {
            handle.send(command);
        }
    }

    pub fn visible<'a>(&'a self, config: &Config) -> Vec<&'a TrayItem> {
        self.items
            .iter()
            .filter(|item| !config.tray.rule(&item.id).hide_icon)
            .collect()
    }

    pub fn view<'a>(
        &'a self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, TrayMessage>> {
        let visible = self.visible(config);
        if visible.is_empty() {
            return None;
        }
        let reveal = self.menu.reveal();

        let handle = button(
            Solid::EllipsisVertical
                .glyph()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(TrayMessage::Menu(MenuMessage::Toggle))
        .into();

        let icons = visible
            .iter()
            .enumerate()
            .map(|(index, item)| item_view(config, item, index, reveal));
        let content = Row::with_children(icons)
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center);

        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            TrayMessage::Menu,
        ))
    }
}

fn click_message(
    action: ClickAction,
    window_app_id: Option<&String>,
    item: &TrayItem,
    index: usize,
) -> Option<TrayMessage> {
    match action {
        ClickAction::ActivateItem => Some(TrayMessage::Activate(item.address.clone())),
        ClickAction::SecondaryActivateItem => Some(TrayMessage::Secondary(item.address.clone())),
        ClickAction::OpenItemMenu => Some(TrayMessage::OpenMenu(index)),
        ClickAction::FocusAppWindow => window_app_id.map(|app_id| TrayMessage::FocusWindow {
            window_app_id: app_id.clone(),
            item_id: item.id.clone(),
        }),
        ClickAction::Ignore => None,
    }
}

fn item_tooltip(item: &TrayItem) -> Tooltip {
    let (title, body) = match &item.tooltip {
        Some(tip) if !tip.title.is_empty() => (tip.title.clone(), tip.body.clone()),
        Some(tip) => (item.name().to_string(), tip.body.clone()),
        None => (item.name().to_string(), String::new()),
    };
    Tooltip::of(TooltipBody::Text { title, body })
}

fn item_view<'a>(
    config: &Config,
    item: &TrayItem,
    index: usize,
    reveal: f32,
) -> Element<'a, TrayMessage> {
    let side = config.font.icon_size_px;
    let glyph: Element<'a, TrayMessage> = match &item.icon {
        Some(source) => native_icon(source, side, reveal),
        None => Solid::Question
            .glyph()
            .size(side)
            .color(config.palette.bar_foreground.scale_alpha(reveal))
            .into(),
    };

    let marked: Element<'a, TrayMessage> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            activity_line(
                config.palette.focus,
                reveal,
                config.widgets.focus_underline_thickness_px,
            ),
        ])
        .spacing(1.0)
        .align_x(Alignment::Center)
        .into()
    } else {
        glyph
    };

    let hover = hoverable(
        marked,
        HoverTarget::Tray(Box::new(item_tooltip(item))),
        TrayMessage::Tooltip,
    );
    let rule = config.tray.rule(&item.id);
    let app_id = rule.window_app_id.as_ref();
    let mut clickable = mouse_area(hover);
    if let Some(message) = click_message(rule.left_click, app_id, item, index) {
        clickable = clickable.on_press(message);
    }
    if let Some(message) = click_message(rule.middle_click, app_id, item, index) {
        clickable = clickable.on_middle_press(message);
    }
    if let Some(message) = click_message(rule.right_click, app_id, item, index) {
        clickable = clickable.on_right_press(message);
    }
    clickable.into()
}
