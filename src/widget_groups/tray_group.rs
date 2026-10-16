pub mod menu_lines;

use std::collections::HashSet;

use iced::{
    Alignment, Element,
    widget::{Column, mouse_area},
};
use tokio::sync::mpsc;

use crate::{
    config::{Config, widget_groups::tray::ClickAction},
    data::source::{
        niri::WindowOwner,
        tray::{self, Status, TrayCommand, TrayItem, TrayUpdate},
    },
    desktop_shell::{
        AppWindowFocus,
        hover_tooltips::{GroupHandle, HoverTarget, PressHints, hoverable},
    },
    widgets::{
        activity_line::activity_line,
        bar_widget::Glyph,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Tooltip, TooltipBody, TrayMenuBody, WholeBody},
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion, HandleFace},
    },
};

#[derive(Default)]
pub struct TrayGroup {
    items: ContinuousReading<Vec<TrayItem>>,
    commands: Option<mpsc::Sender<TrayCommand>>,
    open_submenus: HashSet<i32>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Update(TrayUpdate),
    Expansion(ExpansionMessage),
    Activate(String),
    Secondary(String),
    FocusWindow {
        window_app_id: String,
        address: String,
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
    FocusAppWindow(AppWindowFocus<TrayMessage>),
}

impl TrayGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription() -> iced::Subscription<TrayMessage> {
        tray::subscription().map(TrayMessage::Update)
    }

    pub fn update(&mut self, message: TrayMessage) -> Action {
        match message {
            TrayMessage::Update(TrayUpdate::Connected(commands)) => {
                self.commands = Some(commands);
                Action::None
            }
            TrayMessage::Update(TrayUpdate::Items(items)) => {
                self.items.load(items);
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
                address,
            } => Action::FocusAppWindow(AppWindowFocus {
                owner: WindowOwner::App(window_app_id),
                unmatched: TrayMessage::Activate(address),
            }),
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

    pub fn tooltip_for(&self, address: &str) -> Option<Tooltip> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .find(|item| item.address == address)
            .and_then(|item| self.items.tooltip_with(self.item_tooltip(item)))
    }

    fn item_tooltip(&self, item: &TrayItem) -> TooltipBody {
        let tip = item.tooltip.as_ref();
        let title = tip
            .and_then(|tip| tip.title.clone())
            .unwrap_or_else(|| item.name().to_owned());
        let body = tip.and_then(|tip| tip.body.clone());
        let menu_path = item
            .menu_path
            .as_ref()
            .filter(|_| !item.menu.is_empty())
            .cloned();
        TooltipBody::Whole(match menu_path {
            Some(menu_path) => WholeBody::TrayMenu(TrayMenuBody {
                address: item.address.clone(),
                menu_path,
                title,
                body,
                nodes: item.menu.clone(),
                open_submenus: self.open_submenus.clone(),
            }),
            None => WholeBody::Text { title, body },
        })
    }

    fn command(&self, command: TrayCommand) {
        if let Some(commands) = &self.commands {
            let _ = commands.try_send(command);
        }
    }

    pub fn visible<'a>(&'a self, config: &Config) -> Vec<&'a TrayItem> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .filter(|item| !config.widget_groups.tray.rule(&item.id).hide_icon)
            .collect()
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, TrayMessage> {
        GroupContent::new(
            &self.expansion,
            TrayMessage::Expansion,
            self.visible(config)
                .into_iter()
                .map(|item| GroupItem::element(move |reveal| item_view(config, item, reveal))),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::DotsThreeVertical(Weight::Regular).into(),
            handle: GroupHandle::Tray,
        })
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
            address: item.address.clone(),
        }),
        ClickAction::Ignore => None,
    }
}

fn item_view<'a>(
    config: &Config,
    item: &TrayItem,
    reveal: f32,
) -> Element<'a, GroupEvent<TrayMessage>> {
    let side = config.theme.font.icon_size_px;
    let glyph: Element<'a, GroupEvent<TrayMessage>> = match &item.icon {
        Some(source) => IconHandle::from(source).view(side, reveal),
        None => Glyph::from(Phosphor::Question(Weight::Regular)).view(
            side,
            config.theme.palette.bar_foreground.scale_alpha(reveal),
        ),
    };

    let marked: Element<'a, GroupEvent<TrayMessage>> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            activity_line(
                config.theme.palette.focus,
                reveal,
                config.panel_bar.indicator_thickness,
            ),
        ])
        .spacing(1.0)
        .align_x(Alignment::Center)
        .into()
    } else {
        glyph
    };

    let rule = config.widget_groups.tray.rule(&item.id);
    let app_id = rule.window_app_id.as_ref();
    let left = click_message(rule.left_click, app_id, item);
    let middle = click_message(rule.middle_click, app_id, item);
    let right = click_message(rule.right_click, app_id, item);
    let press_hints = PressHints {
        left: left.as_ref().map(|_| rule.left_click.press_hint()),
        middle: middle.as_ref().map(|_| rule.middle_click.press_hint()),
        right: right.as_ref().map(|_| rule.right_click.press_hint()),
        ..PressHints::default()
    };
    let hover = hoverable(
        marked,
        HoverTarget::Tray(item.address.clone()),
        press_hints,
        GroupEvent::Tooltip,
    );
    let mut clickable = mouse_area(hover);
    if let Some(message) = left {
        clickable = clickable.on_press(GroupEvent::Group(message));
    }
    if let Some(message) = middle {
        clickable = clickable.on_middle_press(GroupEvent::Group(message));
    }
    if let Some(message) = right {
        clickable = clickable.on_right_press(GroupEvent::Group(message));
    }
    clickable.into()
}

impl ClickAction {
    fn press_hint(self) -> &'static str {
        match self {
            ClickAction::ActivateItem => "activate",
            ClickAction::SecondaryActivateItem => "secondary action",
            ClickAction::FocusAppWindow => "focus its window",
            ClickAction::Ignore => "nothing",
        }
    }
}
