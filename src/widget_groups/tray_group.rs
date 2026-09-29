use std::collections::HashSet;

use iced::{
    Alignment, Element,
    widget::{Column, mouse_area},
};

use crate::{
    config::{Config, tray::ClickAction},
    data::source::tray::{self, Status, TrayCommand, TrayHandle, TrayItem, TrayUpdate},
    wayland_layers::tooltip::{HoverTarget, hoverable},
    widgets::{
        activity_line::activity_line,
        bar_widget::Glyph,
        group_item::GroupItem,
        native_icon::native_icon,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Tooltip, TooltipBody, TrayMenuBody},
        widget_group::{ExpansionMessage, GroupEvent, GroupExpansion, WidgetGroup},
    },
};

#[derive(Default)]
pub struct TrayGroup {
    items: Vec<TrayItem>,
    handle: Option<TrayHandle>,
    open_submenus: HashSet<i32>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Update(TrayUpdate),
    Expansion(ExpansionMessage),
    Activate(String),
    Secondary(String),
    FocusWindow {
        window_app_id: String,
        item_id: String,
    },
    MenuItemClicked {
        address: String,
        menu_path: String,
        id: i32,
    },
    ToggleSubmenu(i32),
}

pub enum Action {
    None,
    MenuItemActivated,
    FocusWindow {
        window_app_id: String,
        item_id: String,
    },
}

impl TrayGroup {
    pub fn subscription() -> iced::Subscription<TrayMessage> {
        tray::subscription().map(TrayMessage::Update)
    }

    pub fn update(&mut self, message: TrayMessage) -> Action {
        match message {
            TrayMessage::Update(TrayUpdate::Connected(handle)) => {
                self.handle = Some(handle);
                Action::None
            }
            TrayMessage::Update(TrayUpdate::Items(items)) => {
                self.items = items;
                Action::None
            }
            TrayMessage::Expansion(m) => {
                self.expansion.update(m);
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
            TrayMessage::MenuItemClicked {
                address,
                menu_path,
                id,
            } => {
                self.command(TrayCommand::MenuClick {
                    address,
                    menu_path,
                    id,
                });
                self.open_submenus.clear();
                Action::MenuItemActivated
            }
            TrayMessage::ToggleSubmenu(id) => {
                if !self.open_submenus.remove(&id) {
                    self.open_submenus.insert(id);
                }
                Action::None
            }
        }
    }

    pub fn tooltip_for(&self, address: &str) -> Tooltip {
        self.items
            .iter()
            .find(|item| item.address == address)
            .map_or_else(Tooltip::empty, |item| self.item_tooltip(item))
    }

    fn item_tooltip(&self, item: &TrayItem) -> Tooltip {
        let (title, body) = match &item.tooltip {
            Some(tip) if !tip.title.is_empty() => (tip.title.clone(), tip.body.clone()),
            Some(tip) => (item.name().to_string(), tip.body.clone()),
            None => (item.name().to_string(), String::new()),
        };
        let menu_path = item
            .menu_path
            .as_ref()
            .filter(|_| !item.menu.is_empty())
            .cloned();
        match menu_path {
            Some(menu_path) => Tooltip::of(TooltipBody::TrayMenu(TrayMenuBody {
                address: item.address.clone(),
                menu_path,
                title,
                body,
                nodes: item.menu.clone(),
                open_submenus: self.open_submenus.clone(),
            })),
            None => Tooltip::of(TooltipBody::Text { title, body }),
        }
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

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, TrayMessage> {
        WidgetGroup::new(&self.expansion, TrayMessage::Expansion)
            .icon(Phosphor::DotsThreeVertical(Weight::Regular))
            .items(self.visible(config).into_iter().map(|item| {
                GroupItem::hoverable_element(move |reveal| item_view(config, item, reveal))
            }))
    }
}

fn click_message(
    action: ClickAction,
    window_app_id: Option<&String>,
    item: &TrayItem,
) -> Option<TrayMessage> {
    match action {
        ClickAction::ActivateItem => Some(TrayMessage::Activate(item.address.clone())),
        ClickAction::SecondaryActivateItem => Some(TrayMessage::Secondary(item.address.clone())),
        ClickAction::FocusAppWindow => window_app_id.map(|app_id| TrayMessage::FocusWindow {
            window_app_id: app_id.clone(),
            item_id: item.id.clone(),
        }),
        ClickAction::Ignore => None,
    }
}

fn item_view<'a>(
    config: &Config,
    item: &TrayItem,
    reveal: f32,
) -> Element<'a, GroupEvent<TrayMessage>> {
    let side = config.font.icon_size_px;
    let glyph: Element<'a, GroupEvent<TrayMessage>> = match &item.icon {
        Some(source) => native_icon(source, side, reveal),
        None => Glyph::from(Phosphor::Question(Weight::Regular))
            .view(side, config.palette.bar_foreground.scale_alpha(reveal)),
    };

    let marked: Element<'a, GroupEvent<TrayMessage>> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            activity_line(
                config.palette.focus,
                reveal,
                config.widgets.indicator_thickness_px,
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
        HoverTarget::Tray(item.address.clone()),
        GroupEvent::Tooltip,
    );
    let rule = config.tray.rule(&item.id);
    let app_id = rule.window_app_id.as_ref();
    let mut clickable = mouse_area(hover);
    if let Some(message) = click_message(rule.left_click, app_id, item) {
        clickable = clickable.on_press(GroupEvent::Group(message));
    }
    if let Some(message) = click_message(rule.middle_click, app_id, item) {
        clickable = clickable.on_middle_press(GroupEvent::Group(message));
    }
    if let Some(message) = click_message(rule.right_click, app_id, item) {
        clickable = clickable.on_right_press(GroupEvent::Group(message));
    }
    clickable.into()
}
