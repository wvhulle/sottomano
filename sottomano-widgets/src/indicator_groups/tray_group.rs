use corredo::{
    gauges::activity_line,
    orientation::Axis,
    popovers::{MenuEntry, NestedMenuAction, NestedMenuState},
    theming::primary_color,
};
use iced::{
    Alignment, Element, Task,
    widget::{Column, container, mouse_area},
};
use sottomano_desktop_sinks::desktop_windows_apps::status_notifier_tray_activation::{
    TrayCommand, activate_tray_item, activate_tray_item_with_token,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        status_notifier_tray::{self, Status, TrayHost, TrayItem, TrayItemIcon, TrayUpdate},
        window_edge_placement::WindowOwner,
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
    theme_icons::icon_requests::{IconRequest, ItemThemePath, tray_item_icon_request},
};
use sottomano_toml_config::{
    Config,
    indicator_groups::{
        ElementVisibility,
        tray::{ClickAction, TrayRule},
    },
};

use crate::{
    background_work::logged_failure_task,
    cascading_menus::menu_node_entries::{
        PressDismissal, menu_icon_side_px, menu_node_entries, menu_node_icon_requests,
        press_dismissal,
    },
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::{IndicatorState, IndicatorStateTint},
    },
    hover_cards::{
        MouseGesture, PressAction,
        hover_card_body::{Section, SectionWidget},
    },
    icon_catalog::{
        drawable_icon_catalog::{DrawableIcon, DrawableIconSources, IconCatalog, IconFreshness},
        pixmap_handles::PixmapHandles,
    },
    indicator_groups::{
        indicator_group_content::{
            GroupEvent, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    layer_surfaces::ActivationTokenOutcome,
    widgets::{
        app_window_focus::AppWindowFocus, shared_metrics::ACCENT_STROKE_PX,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

#[cfg(test)]
mod tray_group_tests;

const UNIDENTIFIED_APP_ICON: &str = "application-x-executable";

#[derive(Default)]
pub struct TrayGroup {
    items: SubscribedSnapshot<Vec<TrayItem>>,
    pixmap_handles: PixmapHandles,
    host: Option<TrayHost>,
    pub menu_state: NestedMenuState<i32>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Update(TrayUpdate),
    Expansion(IndicatorGroupExpansionMessage),
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
    Menu(NestedMenuAction<i32>),
}

pub enum Action {
    None,
    Run(Task<TrayMessage>),
    SendWithActivationToken(TrayCommand),
    MenuItemActivated {
        command: TrayCommand,
        dismissal: PressDismissal,
    },
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
        let owner = match &rule.window_app_id {
            Some(window_app_id) => WindowOwner::App(window_app_id.clone()),
            None => match item.owner {
                OwnerProcess::Identified(pid) => WindowOwner::Process(pid),
                OwnerProcess::Unidentified => return Self::NotOpen,
            },
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

    #[must_use]
    pub fn icon_requests(&self, config: &Config) -> Vec<IconRequest> {
        let menu_icon_side_px = menu_icon_side_px(config);
        self.items
            .get()
            .into_iter()
            .flatten()
            .flat_map(|item| {
                let item_request = match &item.icon {
                    TrayItemIcon::Named {
                        icon_name,
                        item_theme_path,
                    } => Some(tray_item_icon_request(icon_name, item_theme_path)),
                    TrayItemIcon::NoIcon | TrayItemIcon::Pixmap(_) => None,
                };
                item_request
                    .into_iter()
                    .chain(menu_node_icon_requests(&item.menu, menu_icon_side_px))
            })
            .chain([unidentified_tray_icon_request()])
            .collect()
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
                let dismissal = self
                    .items
                    .get()
                    .into_iter()
                    .flatten()
                    .find(|item| item.address == address)
                    .map_or(PressDismissal::ClosesMenu, |item| {
                        press_dismissal(&item.menu, id)
                    });
                if dismissal == PressDismissal::ClosesMenu {
                    self.menu_state.close();
                }
                Action::MenuItemActivated {
                    command: TrayCommand::MenuClick {
                        address,
                        menu_path,
                        id,
                    },
                    dismissal,
                }
            }
            TrayMessage::Menu(action) => {
                self.menu_state.perform(action);
                Action::None
            }
        }
    }

    fn item_hover_card(item: &TrayItem) -> Vec<Section> {
        let tip = item.tooltip.as_ref();
        let title = tip
            .and_then(|tip| tip.title.clone())
            .unwrap_or_else(|| item.name().to_owned());
        vec![match tip.and_then(|tip| tip.body.clone()) {
            Some(body) => Section::new(title, SectionWidget::paragraph(body)),
            None => Section::new("Tooltip", SectionWidget::paragraph(title)),
        }]
    }

    #[must_use]
    pub fn menu_entries_named(
        &self,
        item_name: &str,
        config: &Config,
        icon_catalog: &IconCatalog,
    ) -> Vec<MenuEntry<'static, i32, TrayMessage>> {
        let Some(item) = self
            .items
            .get()
            .into_iter()
            .flatten()
            .find(|item| item.name() == item_name)
        else {
            return Vec::new();
        };
        let Some(menu_path) = item.menu_path.clone() else {
            return Vec::new();
        };
        let item_address = item.address.clone();
        let on_activate = move |id| TrayMessage::MenuItemClicked {
            address: item_address.clone(),
            menu_path: menu_path.clone(),
            id,
        };
        menu_node_entries(
            &item.menu,
            config,
            DrawableIconSources {
                icon_catalog,
                pixmap_handles: &self.pixmap_handles,
            },
            &on_activate,
        )
    }

    fn command(&self, command: TrayCommand, token: ActivationTokenOutcome) -> Task<TrayMessage> {
        let Some(host) = self.host.clone() else {
            return Task::none();
        };
        match token {
            ActivationTokenOutcome::Issued(activation_token) => logged_failure_task(
                "tray: activate".to_owned(),
                activate_tray_item_with_token(host, command, activation_token),
            ),
            ActivationTokenOutcome::Unavailable => logged_failure_task(
                "tray: activate".to_owned(),
                activate_tray_item(host, command),
            ),
        }
    }

    #[must_use]
    pub fn visible<'a>(&'a self, config: &Config) -> Vec<&'a TrayItem> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .filter(|item| {
                config.indicator_groups.tray.rule(&item.id).icon == ElementVisibility::Shown
            })
            .collect()
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        icon_catalog: &'a IconCatalog,
        window_matches: &dyn Fn(&WindowOwner) -> bool,
    ) -> IndicatorGroupContent<'a> {
        let group_indicators: Vec<_> = self
            .visible(config)
            .into_iter()
            .map(|item| {
                let item_window = TrayItemWindow::resolve(
                    &config.indicator_groups.tray.rule(&item.id),
                    item,
                    window_matches,
                );
                let item_rule = config.indicator_groups.tray.rule(&item.id);
                let press_actions = item_press_actions(&item_rule, &item_window);
                GroupIndicator::composed(
                    item.name().to_owned(),
                    IndicatorIcon::Absent,
                    IndicatorValue::drawn(move |reveal| {
                        let icon_sources = DrawableIconSources {
                            icon_catalog,
                            pixmap_handles: &self.pixmap_handles,
                        };
                        item_view(config, icon_sources, item, &item_window, reveal)
                    }),
                    Box::new(move |_| self.items.hover_card_with(Self::item_hover_card(item))),
                )
                .with_button_actions(press_actions)
            })
            .collect();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Tray,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Tray.title(),
                Phosphor::DotsThreeVertical(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            TrayMessage::Expansion,
            group_indicators,
        )
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

fn item_press_actions(rule: &TrayRule, item_window: &TrayItemWindow) -> Vec<PressAction> {
    [
        (MouseGesture::LeftClick, rule.left_click),
        (MouseGesture::MiddleClick, rule.middle_click),
        (MouseGesture::RightClick, rule.right_click),
    ]
    .into_iter()
    .filter(|(_, click_action)| *click_action != ClickAction::Ignore)
    .map(|(gesture, click_action)| PressAction::new(gesture, click_hint(click_action, item_window)))
    .collect()
}

fn unidentified_tray_icon_request() -> IconRequest {
    tray_item_icon_request(UNIDENTIFIED_APP_ICON, &ItemThemePath::Unset)
}

fn tray_item_drawable(
    icon_sources: DrawableIconSources<'_>,
    tray_item_icon: &TrayItemIcon,
) -> Option<DrawableIcon> {
    let catalog_drawable =
        |request: &IconRequest| icon_sources.icon_catalog.drawable_icon(request).cloned();
    let unidentified_drawable = || catalog_drawable(&unidentified_tray_icon_request());
    match tray_item_icon {
        TrayItemIcon::Pixmap(pixels) => Some(DrawableIcon {
            handle: icon_sources.pixmap_handles.pixmap_handle(pixels),
            design_side_px: pixels.width().max(pixels.height()) as f32,
            freshness: IconFreshness::Current,
        }),
        TrayItemIcon::Named {
            icon_name,
            item_theme_path,
        } => catalog_drawable(&tray_item_icon_request(icon_name, item_theme_path))
            .or_else(unidentified_drawable),
        TrayItemIcon::NoIcon => unidentified_drawable(),
    }
}

fn item_view<'a>(
    config: &Config,
    icon_sources: DrawableIconSources<'_>,
    item: &TrayItem,
    item_window: &TrayItemWindow,
    reveal: f32,
) -> Element<'a, GroupEvent> {
    let side = config.theme.font.icon_size_px;
    let tint = IndicatorStateTint {
        state: IndicatorState::Idle,
        opacity: reveal,
    };
    let glyph: Element<'a, GroupEvent> = match tray_item_drawable(icon_sources, &item.icon) {
        Some(drawable_icon) => {
            drawable_icon.view_with_symbolic_tint(side, reveal, move |theme| tint.color(theme))
        }
        None => IconGlyph::from(Phosphor::Question(Weight::Regular))
            .view(side, move |theme| tint.color(theme)),
    };

    let marked: Element<'a, GroupEvent> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            container(activity_line(
                Axis::Horizontal,
                primary_color,
                reveal,
                ACCENT_STROKE_PX,
            ))
            .width(side)
            .into(),
        ])
        .align_x(Alignment::Center)
        .into()
    } else {
        glyph
    };

    let rule = config.indicator_groups.tray.rule(&item.id);
    let left = click_message(rule.left_click, item_window, item);
    let middle = click_message(rule.middle_click, item_window, item);
    let right = click_message(rule.right_click, item_window, item);
    let mut clickable = mouse_area(marked);
    if let Some(message) = left {
        clickable = clickable.on_press(GroupEvent::from(message));
    }
    if let Some(message) = middle {
        clickable = clickable.on_middle_press(GroupEvent::from(message));
    }
    if let Some(message) = right {
        clickable = clickable.on_right_press(GroupEvent::from(message));
    }
    clickable.into()
}
