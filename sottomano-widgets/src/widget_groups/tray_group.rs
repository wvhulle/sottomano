pub mod menu_lines;

use std::{collections::HashSet, sync::Arc};

use iced::{
    Alignment, Element,
    border::Radius,
    widget::{Column, mouse_area},
};
use sottomano_config::{Config, widget_groups::tray::ClickAction};
use sottomano_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        niri_window_placement::WindowOwner,
        status_notifier_tray::{self, Status, TrayCommand, TrayItem, TrayUpdate},
    },
};
use system_tray::error;
use tokio::sync::mpsc;

use self::menu_lines::TrayMenuLines;
use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget, PressHints, hoverable},
    widgets::{
        activity_line::activity_line,
        app_window_focus::AppWindowFocus,
        configured_theme::primary_color,
        context_menu::context_menu,
        group_item::{GroupItem, ItemHover, ItemIcon, ItemIdentity, ItemValue},
        icon_glyph::IconGlyph,
        icon_handle::PixmapHandles,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody, tooltip_source_delivery::Tooltip},
        widget_group::{
            ExpansionMessage, GroupContent, GroupEvent, GroupExpansion, HandleFace, WidgetGroup,
        },
        widget_state::{StateTint, WidgetState},
    },
};

impl From<Arc<error::Error>> for Section {
    fn from(error: Arc<error::Error>) -> Self {
        Section::error(error)
    }
}

#[derive(Default)]
pub struct TrayGroup {
    items: LatestSnapshot<Vec<TrayItem>, HidesFetchFailures>,
    pixmap_handles: PixmapHandles,
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

impl WidgetGroup for TrayGroup {
    const TITLE: &'static str = "Tray";
}

impl TrayGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription() -> iced::Subscription<TrayMessage> {
        iced::Subscription::run(status_notifier_tray::updates).map(TrayMessage::Update)
    }

    pub fn update(&mut self, message: TrayMessage, clock: Clock) -> Action {
        match message {
            TrayMessage::Update(TrayUpdate::Connected(commands)) => {
                self.commands = Some(commands);
                Action::None
            }
            TrayMessage::Update(TrayUpdate::Items(items)) => {
                self.pixmap_handles = self
                    .pixmap_handles
                    .refreshed(items.iter().flat_map(TrayItem::pixmaps));
                self.items
                    .receive(SnapshotDelivery::live_push(Ok(items)), clock);
                Action::None
            }
            TrayMessage::Update(TrayUpdate::HostFailed(failure)) => {
                self.commands = None;
                self.items
                    .receive(SnapshotDelivery::live_push(Err(failure)), clock);
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

    #[must_use]
    pub fn tooltip_for(&self, address: &str) -> Option<Tooltip> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .find(|item| item.address == address)
            .and_then(|item| self.items.tooltip_with(Self::item_tooltip(item)))
    }

    fn item_tooltip(item: &TrayItem) -> TooltipBody {
        let tip = item.tooltip.as_ref();
        let title = tip
            .and_then(|tip| tip.title.clone())
            .unwrap_or_else(|| item.name().to_owned());
        TooltipBody::from(match tip.and_then(|tip| tip.body.clone()) {
            Some(body) => Section::detailed_note(title, body),
            None => Section::note(title),
        })
    }

    #[must_use]
    pub fn menu_panel<'a>(
        &'a self,
        address: &str,
        config: &'a Config,
        popup_corners: Radius,
    ) -> Option<Element<'a, TrayMessage>> {
        let item = self
            .items
            .get()
            .into_iter()
            .flatten()
            .find(|item| item.address == address && !item.menu.is_empty())?;
        let menu_path = item.menu_path.clone()?;
        let item_address = item.address.clone();
        let on_activate = move |id| TrayMessage::MenuItemClicked {
            address: item_address.clone(),
            menu_path: menu_path.clone(),
            id,
        };
        let lines = TrayMenuLines {
            config,
            pixmap_handles: &self.pixmap_handles,
            open: &self.open_submenus,
            on_activate: &on_activate,
            on_toggle: &TrayMessage::ToggleSubmenu,
        }
        .lines(&item.menu);
        Some(context_menu(config, popup_corners, lines))
    }

    fn command(&self, command: TrayCommand) {
        if let Some(commands) = &self.commands {
            let _ = commands.try_send(command);
        }
    }

    #[must_use]
    pub fn visible<'a>(&'a self, config: &Config) -> Vec<&'a TrayItem> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .filter(|item| !config.widget_groups.tray.rule(&item.id).hide_icon)
            .collect()
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, TrayMessage> {
        Self::expandable_group(
            &self.expansion,
            TrayMessage::Expansion,
            self.visible(config).into_iter().map(|item| {
                GroupItem::composed(
                    ItemIdentity {
                        name: item.name().to_owned().into(),
                        icon: ItemIcon::Absent,
                    },
                    ItemHover::ByChildren {
                        row_target: HoverTarget::Tray(item.address.clone()),
                    },
                    ItemValue::drawn(move |reveal| {
                        item_view(config, &self.pixmap_handles, item, reveal)
                    }),
                )
            }),
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
    pixmap_handles: &PixmapHandles,
    item: &TrayItem,
    reveal: f32,
) -> Element<'a, GroupEvent<TrayMessage>> {
    let side = config.theme.font.icon_size_px;
    let tint = StateTint {
        state: WidgetState::Idle,
        opacity: reveal,
    };
    let glyph: Element<'a, GroupEvent<TrayMessage>> = item.icon.as_ref().map_or_else(
        || {
            IconGlyph::from(Phosphor::Question(Weight::Regular))
                .view(side, move |theme| tint.color(theme))
        },
        |source| pixmap_handles.icon_handle(source).view(side, reveal),
    );

    let marked: Element<'a, GroupEvent<TrayMessage>> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            activity_line(primary_color, reveal, config.panel_bar.indicator_thickness),
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
