use corredo::popovers::{MenuEntry, NestedMenuAction, NestedMenuState};
use iced::Task;
use sottomano_desktop_sinks::desktop_windows_apps::status_notifier_tray_activation::{
    TrayCommand, activate_tray_item, activate_tray_item_with_token,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        status_notifier_tray::{
            self, TrayHost, TrayItem, TrayItemIcon, TrayMenuPath, TrayTooltip, TrayUpdate,
        },
        window_edge_placement::WindowOwner,
    },
    theme_icons::icon_requests::{IconRequest, tray_item_icon_request},
};
use sottomano_toml_config::{Config, indicator_groups::ElementVisibility};

use crate::{
    background_work::logged_failure_task,
    cascading_menus::menu_node_entries::{
        PressDismissal, menu_icon_side_px, menu_node_entries, menu_node_icon_requests,
        press_dismissal,
    },
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
    icon_catalog::{
        drawable_icon_catalog::{DrawableIconSources, IconCatalog},
        pixmap_handles::PixmapHandles,
    },
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    layer_surfaces::ActivationTokenOutcome,
    widgets::{app_window_focus::AppWindowFocus, subscribed_snapshot::SubscribedSnapshot},
};

#[cfg(test)]
mod tray_group_tests;
mod tray_item_view;

use self::tray_item_view::{
    TrayItemWindow, item_press_actions, item_view, unidentified_tray_icon_request,
};

#[derive(Default)]
enum TrayHostConnection {
    #[default]
    Disconnected,
    Connected(TrayHost),
}

#[derive(Default)]
pub struct TrayGroup {
    items: SubscribedSnapshot<Vec<TrayItem>>,
    pixmap_handles: PixmapHandles,
    host: TrayHostConnection,
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
                self.host = TrayHostConnection::Connected(host);
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
                self.host = TrayHostConnection::Disconnected;
                self.items
                    .receive(SnapshotDelivery::live_push(Err(failure)), clock);
                Action::None
            }
            TrayMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
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
        vec![match &item.tooltip {
            TrayTooltip::NoTooltip => {
                Section::new("Tooltip", SectionWidget::paragraph(item.name.clone()))
            }
            TrayTooltip::TitleOnly(title) => {
                Section::new("Tooltip", SectionWidget::paragraph(title.clone()))
            }
            TrayTooltip::BodyOnly(body) => {
                Section::new(item.name.clone(), SectionWidget::paragraph(body.clone()))
            }
            TrayTooltip::TitledBody { title, body } => {
                Section::new(title.clone(), SectionWidget::paragraph(body.clone()))
            }
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
            .find(|item| item.name == item_name)
        else {
            return Vec::new();
        };
        let TrayMenuPath::Exported(menu_path) = item.menu_path.clone() else {
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
        let TrayHostConnection::Connected(host) = &self.host else {
            return Task::none();
        };
        let host = host.clone();
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
    pub fn visible<'group>(&'group self, config: &Config) -> Vec<&'group TrayItem> {
        self.items
            .get()
            .into_iter()
            .flatten()
            .filter(|item| {
                config.indicator_groups.tray.rule(&item.id).icon == ElementVisibility::Shown
            })
            .collect()
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
        icon_catalog: &'view IconCatalog,
        window_matches: &dyn Fn(&WindowOwner) -> bool,
    ) -> IndicatorGroupContent<'view> {
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
                    item.name.clone(),
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
