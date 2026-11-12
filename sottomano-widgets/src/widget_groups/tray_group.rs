use std::{collections::HashSet, sync::Arc};

use iced::{
    Alignment, Element, Task,
    border::Radius,
    widget::{Column, container, mouse_area},
};
use sottomano_desktop_sinks::desktop_windows_apps::status_notifier_tray_activation::{
    TrayCommand, activate_tray_item, activate_tray_item_with_token,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        status_notifier_tray::{self, Status, TrayHost, TrayItem, TrayUpdate},
        window_edge_placement::WindowOwner,
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_toml_config::{
    Config,
    widget_groups::{
        ElementVisibility,
        tray::{ClickAction, TrayRule},
    },
};
use system_tray::error;

use crate::{
    clock::Clock,
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
    },
    hover_cards::{
        HoverTarget, PressHints,
        hover_card_body::{HoverCardBody, Section, hover_card_source_delivery::HoverCard},
        hoverable,
    },
    layer_surfaces::ActivationTokenOutcome,
    widget_groups::{
        widget_group_content::{
            GroupEvent, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        activity_line::activity_line,
        app_window_focus::AppWindowFocus,
        application_menu_lines::ApplicationMenuLines,
        configured_theme::primary_color,
        context_menu::context_menu,
        icon_glyph::IconGlyph,
        icon_handle::PixmapHandles,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        shared_metrics::ACCENT_STROKE_PX,
    },
};

#[derive(Default)]
pub struct TrayGroup {
    items: LatestSnapshot<Vec<TrayItem>, HidesFetchFailures>,
    pixmap_handles: PixmapHandles,
    host: Option<TrayHost>,
    open_submenus: HashSet<i32>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Update(TrayUpdate),
    Expansion(WidgetGroupExpansionMessage),
    Activate(String),
    Secondary(String),
    FocusWindow {
        owner: WindowOwner,
        address: String,
    },
    Send {
        command: TrayCommand,
        token: ActivationTokenOutcome,
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
    Run(Task<TrayMessage>),
    SendWithActivationToken(TrayCommand),
    MenuItemActivated(TrayCommand),
    FocusAppWindow(AppWindowFocus<TrayMessage>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TrayItemWindow {
    Open(WindowOwner),
    NotOpen,
}

impl TrayItemWindow {
    fn resolve(
        rule: &TrayRule,
        item: &TrayItem,
        window_matches: &dyn Fn(&WindowOwner) -> bool,
    ) -> Self {
        let owner = match (&rule.window_app_id, item.owner) {
            (Some(window_app_id), _) => WindowOwner::App(window_app_id.clone()),
            (None, OwnerProcess::Identified(pid)) => WindowOwner::Process(pid),
            (None, OwnerProcess::Unidentified) => return Self::NotOpen,
        };
        if window_matches(&owner) {
            Self::Open(owner)
        } else {
            Self::NotOpen
        }
    }
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
            TrayMessage::Update(TrayUpdate::Connected(host)) => {
                self.host = Some(host);
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
                self.host = None;
                self.items
                    .receive(SnapshotDelivery::live_push(Err(failure)), clock);
                Action::None
            }
            TrayMessage::Expansion(m) => {
                self.expansion.update(m);
                Action::None
            }
            TrayMessage::Activate(address) => {
                tracing::debug!("tray {address}: click activates the item");
                Action::SendWithActivationToken(TrayCommand::Activate(address))
            }
            TrayMessage::Secondary(address) => {
                Action::SendWithActivationToken(TrayCommand::Secondary(address))
            }
            TrayMessage::FocusWindow { owner, address } => {
                tracing::debug!("tray {address}: click focuses the window of {owner:?}");
                Action::FocusAppWindow(AppWindowFocus {
                    owner,
                    unmatched: TrayMessage::Activate(address),
                })
            }
            TrayMessage::Send { command, token } => Action::Run(self.command(command, token)),
            TrayMessage::MenuItemClicked {
                address,
                menu_path,
                id,
            } => {
                self.open_submenus.clear();
                Action::MenuItemActivated(TrayCommand::MenuClick {
                    address,
                    menu_path,
                    id,
                })
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
    pub fn hover_card_for(&self, address: &str) -> Option<HoverCard> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .find(|item| item.address == address)
            .and_then(|item| self.items.hover_card_with(Self::item_hover_card(item)))
    }

    fn item_hover_card(item: &TrayItem) -> HoverCardBody {
        let tip = item.tooltip.as_ref();
        let title = tip
            .and_then(|tip| tip.title.clone())
            .unwrap_or_else(|| item.name().to_owned());
        HoverCardBody::from(match tip.and_then(|tip| tip.body.clone()) {
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
        let lines = ApplicationMenuLines {
            config,
            pixmap_handles: &self.pixmap_handles,
            open: &self.open_submenus,
            on_activate: &on_activate,
            on_toggle: &TrayMessage::ToggleSubmenu,
        }
        .lines(&item.menu);
        Some(context_menu(config, popup_corners, lines))
    }

    fn command(&self, command: TrayCommand, token: ActivationTokenOutcome) -> Task<TrayMessage> {
        let Some(host) = self.host.clone() else {
            return Task::none();
        };
        match token {
            ActivationTokenOutcome::Issued(activation_token) => Task::future(
                activate_tray_item_with_token(host, command, activation_token),
            )
            .discard(),
            ActivationTokenOutcome::Unavailable => {
                Task::future(activate_tray_item(host, command)).discard()
            }
        }
    }

    #[must_use]
    pub fn visible<'a>(&'a self, config: &Config) -> Vec<&'a TrayItem> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .filter(|item| {
                config.widget_groups.tray.rule(&item.id).icon == ElementVisibility::Shown
            })
            .collect()
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        window_matches: &dyn Fn(&WindowOwner) -> bool,
    ) -> WidgetGroupContent<'a, TrayMessage> {
        let group_items: Vec<_> = self
            .visible(config)
            .into_iter()
            .map(|item| {
                let item_window = TrayItemWindow::resolve(
                    &config.widget_groups.tray.rule(&item.id),
                    item,
                    window_matches,
                );
                GroupItem::composed(
                    ComposedGroupItemFace {
                        name: item.name().to_owned().into(),
                        icon: GroupItemIcon::Absent,
                    },
                    GroupItemHoverArea::ByChildren {
                        row_target: HoverTarget::Tray(item.address.clone()),
                    },
                    GroupItemValue::drawn(move |reveal| {
                        item_view(config, &self.pixmap_handles, item, &item_window, reveal)
                    }),
                )
            })
            .collect();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Tray,
            &self.expansion,
            TrayMessage::Expansion,
            group_items,
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::DotsThreeVertical(Weight::Regular).into(),
        })
    }
}

fn click_message(
    action: ClickAction,
    item_window: &TrayItemWindow,
    item: &TrayItem,
) -> Option<TrayMessage> {
    match (action, item_window) {
        (ClickAction::ActivateItem, _) | (ClickAction::FocusAppWindow, TrayItemWindow::NotOpen) => {
            Some(TrayMessage::Activate(item.address.clone()))
        }
        (ClickAction::SecondaryActivateItem, _) => {
            Some(TrayMessage::Secondary(item.address.clone()))
        }
        (ClickAction::FocusAppWindow, TrayItemWindow::Open(owner)) => {
            Some(TrayMessage::FocusWindow {
                owner: owner.clone(),
                address: item.address.clone(),
            })
        }
        (ClickAction::Ignore, _) => None,
    }
}

const fn click_hint(action: ClickAction, item_window: &TrayItemWindow) -> &'static str {
    match (action, item_window) {
        (ClickAction::FocusAppWindow, TrayItemWindow::NotOpen) => {
            ClickAction::ActivateItem.press_hint()
        }
        _ => action.press_hint(),
    }
}

fn item_view<'a>(
    config: &Config,
    pixmap_handles: &PixmapHandles,
    item: &TrayItem,
    item_window: &TrayItemWindow,
    reveal: f32,
) -> Element<'a, GroupEvent<TrayMessage>> {
    let side = config.theme.font.icon_size_px;
    let tint = ReadingStateTint {
        state: ReadingDisplayState::Idle,
        opacity: reveal,
    };
    let glyph: Element<'a, GroupEvent<TrayMessage>> = item.icon.as_ref().map_or_else(
        || {
            IconGlyph::from(Phosphor::Question(Weight::Regular))
                .view(side, move |theme| tint.color(theme))
        },
        |source| {
            pixmap_handles
                .icon_handle(source)
                .view_with_symbolic_tint(side, reveal, move |theme| tint.color(theme))
        },
    );

    let marked: Element<'a, GroupEvent<TrayMessage>> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            container(activity_line(primary_color, reveal, ACCENT_STROKE_PX))
                .width(side)
                .into(),
        ])
        .align_x(Alignment::Center)
        .into()
    } else {
        glyph
    };

    let rule = config.widget_groups.tray.rule(&item.id);
    let left = click_message(rule.left_click, item_window, item);
    let middle = click_message(rule.middle_click, item_window, item);
    let right = click_message(rule.right_click, item_window, item);
    let press_hints = PressHints {
        left: left
            .as_ref()
            .map(|_| click_hint(rule.left_click, item_window)),
        middle: middle
            .as_ref()
            .map(|_| click_hint(rule.middle_click, item_window)),
        right: right
            .as_ref()
            .map(|_| click_hint(rule.right_click, item_window)),
        ..PressHints::default()
    };
    let hover = hoverable(
        marked,
        HoverTarget::Tray(item.address.clone()),
        press_hints,
        GroupEvent::HoverCard,
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

#[cfg(test)]
mod tests {
    use corredo::primitives::reveal_clip::reveal_x;
    use iced::{
        Settings, Size,
        widget::{row, text},
    };
    use iced_test::Simulator;
    use sottomano_desktop_sources::{
        desktop_windows_apps::{
            status_notifier_tray::{Status, TrayItem},
            window_edge_placement::WindowOwner,
        },
        system_protocol_clients::dbus_desktop_buses::OwnerProcess,
    };
    use sottomano_toml_config::{
        Config,
        widget_groups::tray::{ClickAction, TrayRule},
    };

    use super::{TrayItemWindow, TrayMessage, click_hint, click_message, item_view};
    use crate::widgets::icon_handle::PixmapHandles;

    const PANEL_WIDTH: f32 = 800.0;
    const MARKER_AFTER_ITEM: &str = "marker after the tray item";

    fn item_needing_attention() -> TrayItem {
        TrayItem {
            address: ":1.4167".to_owned(),
            owner: OwnerProcess::Identified(4167),
            id: "chat".to_owned(),
            title: None,
            status: Status::NeedsAttention,
            tooltip: None,
            menu_path: None,
            icon: None,
            menu: Vec::new(),
        }
    }

    #[test]
    fn an_item_needing_attention_keeps_a_finite_width_while_revealed() {
        let config = Config::default();
        let pixmap_handles = PixmapHandles::default();
        let item = item_needing_attention();
        let revealed = reveal_x(
            item_view(
                &config,
                &pixmap_handles,
                &item,
                &TrayItemWindow::NotOpen,
                1.0,
            ),
            1.0,
        );
        let mut simulator = Simulator::with_size(
            Settings::default(),
            Size::new(PANEL_WIDTH, 40.0),
            row![revealed, text(MARKER_AFTER_ITEM)],
        );
        let marker = simulator
            .find(MARKER_AFTER_ITEM)
            .expect("the marker is laid out")
            .bounds();
        assert!(
            marker.x.is_finite() && marker.x < PANEL_WIDTH,
            "the tray item pushed the marker to x = {}",
            marker.x
        );
    }

    fn every_window_matches(_owner: &WindowOwner) -> bool {
        true
    }

    fn no_window_matches(_owner: &WindowOwner) -> bool {
        false
    }

    #[test]
    fn item_without_override_is_matched_by_its_process() {
        let item = item_needing_attention();
        assert_eq!(
            TrayItemWindow::resolve(&TrayRule::default(), &item, &every_window_matches),
            TrayItemWindow::Open(WindowOwner::Process(4167))
        );
    }

    #[test]
    fn window_app_id_overrides_the_process() {
        let item = item_needing_attention();
        let overriding_rule = TrayRule {
            window_app_id: Some("signal".to_owned()),
            ..TrayRule::default()
        };
        assert_eq!(
            TrayItemWindow::resolve(&overriding_rule, &item, &every_window_matches),
            TrayItemWindow::Open(WindowOwner::App("signal".to_owned()))
        );
    }

    #[test]
    fn item_with_unidentified_process_has_no_open_window() {
        let item = TrayItem {
            owner: OwnerProcess::Unidentified,
            ..item_needing_attention()
        };
        assert_eq!(
            TrayItemWindow::resolve(&TrayRule::default(), &item, &every_window_matches),
            TrayItemWindow::NotOpen
        );
    }

    #[test]
    fn item_without_matching_window_has_no_open_window() {
        let item = item_needing_attention();
        assert_eq!(
            TrayItemWindow::resolve(&TrayRule::default(), &item, &no_window_matches),
            TrayItemWindow::NotOpen
        );
    }

    #[test]
    fn focus_click_on_open_window_focuses_it() {
        let item = item_needing_attention();
        let item_window = TrayItemWindow::Open(WindowOwner::Process(4167));
        assert!(matches!(
            click_message(ClickAction::FocusAppWindow, &item_window, &item),
            Some(TrayMessage::FocusWindow {
                owner: WindowOwner::Process(4167),
                ..
            })
        ));
        assert_eq!(
            click_hint(ClickAction::FocusAppWindow, &item_window),
            "focus its window"
        );
    }

    #[test]
    fn focus_click_without_open_window_activates() {
        let item = item_needing_attention();
        assert!(matches!(
            click_message(ClickAction::FocusAppWindow, &TrayItemWindow::NotOpen, &item),
            Some(TrayMessage::Activate(_))
        ));
        assert_eq!(
            click_hint(ClickAction::FocusAppWindow, &TrayItemWindow::NotOpen),
            "activate"
        );
    }
}
