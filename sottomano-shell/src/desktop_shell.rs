use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

use corredo::theming::text_color;
use iced::{Color, Element, Length, Task, task::Handle, theme::Style, widget::Space, window::Id};
use iced_controller_diagram::ControllerArtwork;
use sottomano_compositor_model::{
    compositor_output_facts, compositor_styling_model::EdgeShadowRendering,
    compositor_window_state::CompositorWindowState,
};
use sottomano_compositors::compositor_connection::CompositorKind;
use sottomano_desktop_sinks::{
    compositor_rule_files::CompositorRulesFile,
    desktop_notifications::alert_sound_player::SoundTheme,
};
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{
        app_launch_history::LaunchTracker, configured_workspace_order::ConfiguredWorkspaces,
        desktop_entries::DesktopEntries, desktop_entry_changes, workspace_window_snapshot,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{Config, ConfigFile, LoadError, SecretPaths, screen_edges::Position};
use sottomano_widgets::{
    clock::Clock,
    group_indicators::group_indicator::HoverFetch,
    group_menus::group_menu_state::{GroupMenuMessage, OpenGroupMenu},
    hover_cards::HoverCardHost,
    icon_catalog::{drawable_icon_catalog::IconCatalog, favicons::Favicons},
    load_tally::{WorkKind, WorkMeasurement, record_update, record_view},
    notification_toasts::{NotificationLayer, ToastSurfaceStyle},
    widgets::{
        configured_theme::configured_theme,
        text::point_generic_font_families_at,
        typing_under_cards::{CardBackgroundFade, TypingUnderCards},
    },
};

use crate::{
    compositor_updates::{SwitchHeatEffect, report_missing_compositor, switch_heat_effect},
    indicator_groups::ShellIndicatorGroups,
    monitors::{
        edge_backdrops::backdrop_view,
        monitor_surfaces::{Surface, SurfaceRole, query_outputs},
    },
    notification_updates::{AlertRescanNeed, GroupAlertScan, alert_rescan_need},
    pointer_drag::ShellDrag,
    shell_message::{Message, ShellInteraction},
    sqlite_app_launches,
};

pub struct DesktopShell {
    pub wayland: Option<wayland_client::Connection>,
    pub desktop_buses: Option<DesktopBuses>,
    pub config_file: ConfigFile,
    pub clock: Clock,
    pub config: Config,
    pub theme: iced::Theme,
    pub indicator_groups: ShellIndicatorGroups,
    pub launch_tracker: LaunchTracker,
    pub compositor: CompositorWindowState,
    pub window_snapshot: workspace_window_snapshot::Snapshot,
    pub outputs: HashMap<String, compositor_output_facts::OutputInfo>,
    pub surfaces: BTreeMap<Id, Surface>,
    pub backdrops: BTreeMap<Id, Position>,
    pub edge_shadow_rendering: EdgeShadowRendering,
    pub sidebar_widths: HashMap<Id, f32>,
    pub hover_card: HoverCardHost,
    pub active_hover_fetch: HoverFetch,
    pub notifications: NotificationLayer<ShellInteraction>,
    pub desktop_entries: Arc<DesktopEntries>,
    pub icon_catalog: IconCatalog,
    pub favicons: Favicons,
    pub controller_artwork: ControllerArtwork,
    pub pointer_drag: ShellDrag,
    pub typing_under_cards: TypingUnderCards,
    pub spring_load: Option<Handle>,
    pub config_home: PathBuf,
    pub configured_workspaces: Option<ConfiguredWorkspaces>,
}

impl DesktopShell {
    pub fn new(
        wayland: Option<wayland_client::Connection>,
        config: Config,
        config_file: ConfigFile,
        secrets: SecretPaths,
        compositor_rules: CompositorRulesFile,
        loaded: Result<Box<Config>, Arc<LoadError>>,
    ) -> (Self, Task<Message>) {
        let _span = tracing::debug_span!("desktop shell init").entered();
        point_generic_font_families_at(&config.theme);
        let clock = Clock::real();
        let config_home = compositor_rules.config_home.clone();
        let shell = Self {
            indicator_groups: ShellIndicatorGroups::new(&config, secrets, compositor_rules, clock),
            launch_tracker: LaunchTracker::default(),
            compositor: CompositorWindowState::default(),
            window_snapshot: workspace_window_snapshot::Snapshot::default(),
            outputs: HashMap::new(),
            surfaces: BTreeMap::new(),
            backdrops: BTreeMap::new(),
            edge_shadow_rendering: CompositorKind::detected().map_or(
                EdgeShadowRendering::SelfDrawnFalloff,
                CompositorKind::edge_shadow_rendering,
            ),
            sidebar_widths: HashMap::new(),
            hover_card: HoverCardHost::default(),
            active_hover_fetch: HoverFetch::NothingToFetch,
            notifications: NotificationLayer::default(),
            desktop_entries: Arc::new(DesktopEntries::default()),
            icon_catalog: IconCatalog::default(),
            favicons: Favicons::default(),
            controller_artwork: ControllerArtwork::decoded(),
            pointer_drag: ShellDrag::Idle,
            typing_under_cards: TypingUnderCards::new(clock.instant, &config.overlay_surfaces),
            spring_load: None,
            config_home,
            configured_workspaces: None,
            theme: configured_theme(&config.theme.palette),
            wayland,
            desktop_buses: None,
            config,
            config_file,
            clock,
        };
        let init = Task::batch([
            Task::done(Message::ConfigLoaded(loaded)),
            Task::perform(desktop_entry_changes::load(), Message::DesktopEntriesLoaded),
            Task::perform(sqlite_app_launches::load(), Message::TrackerLoaded),
            shell.indicator_groups.load(clock),
            shell.read_configured_workspace_order(),
            report_missing_compositor(),
            Task::perform(
                blocking("sound theme", SoundTheme::read_session),
                Message::SoundThemeLoaded,
            ),
            query_outputs(),
        ]);
        (shell, init)
    }

    #[must_use]
    pub fn namespace() -> String {
        "sottomano".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let kind = message_work_kind(&message);
        let _span = tracing::trace_span!("update", kind = %kind).entered();
        let measurement = WorkMeasurement::start();
        let updated = self.measured_update(message);
        record_update(kind, measurement.finish());
        updated
    }

    fn measured_update(&mut self, message: Message) -> Task<Message> {
        self.clock = Clock::real();
        let hover_card_target_before = self.hover_card.target().cloned();
        let hovered_before = self.hovered_indicator_target();
        let heat_effect = switch_heat_effect(&message);
        let alert_rescan = alert_rescan_need(&message);
        let dispatched = self.dispatch_message(message);
        self.indicator_groups
            .session_focus_group
            .refresh_daily_totals(&self.config.indicator_groups.session_focus, self.clock);
        let reheated = if heat_effect == SwitchHeatEffect::Reheats {
            self.indicator_groups
                .reheat_switch_heat(&self.config, &self.compositor, self.clock)
        } else {
            Task::none()
        };
        if self.hover_card.target() != hover_card_target_before.as_ref() {
            let _closed = self
                .indicator_groups
                .group_menus
                .update(GroupMenuMessage::Closed);
            self.indicator_groups.tray_group.menu_state.close();
            self.indicator_groups.window_menu_bar.menu_state.close();
            self.indicator_groups.app_launcher_menu.close_menu();
        }
        let hover_indicators = if self.hovered_indicator_target() == hovered_before {
            Task::none()
        } else {
            self.retarget_hover_indicators()
        };
        let open_menu = self
            .open_group_menu()
            .map(|topic| self.group_menu_of(topic).open_group_menu());
        let widget_toasts = match alert_rescan {
            AlertRescanNeed::Rescans => self.rescan_group_alerts(open_menu),
            AlertRescanNeed::Skips => {
                self.indicator_groups
                    .group_menus
                    .track_open_menu(open_menu, self.clock.instant);
                Task::none()
            }
        };
        let overlay_surfaces = self.reconcile_overlay_surfaces();
        Task::batch([
            dispatched,
            reheated,
            hover_indicators,
            widget_toasts,
            overlay_surfaces,
        ])
    }

    fn rescan_group_alerts(&mut self, open_menu: Option<OpenGroupMenu>) -> Task<Message> {
        let GroupAlertScan {
            toast_requests,
            panel_raises,
        } = self.scan_group_alerts();
        self.indicator_groups.group_menus.track_source_demand(
            open_menu,
            panel_raises,
            self.clock.instant,
        );
        self.notifications
            .show_widget_toasts(
                toast_requests,
                &self
                    .indicator_groups
                    .toast_context(&self.compositor, &self.config, self.clock),
            )
            .map(Message::from)
    }

    fn toast_background_fade(&self, output: &str) -> CardBackgroundFade {
        self.typing_under_cards.background_fade_on(
            output,
            &self.compositor,
            self.clock.instant,
            &self.config.overlay_surfaces,
        )
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        let measurement = WorkMeasurement::start();
        let viewed = self.interaction_view(window).map(Message::from);
        record_view(measurement.finish());
        viewed
    }

    fn interaction_view(&self, window: Id) -> Element<'_, ShellInteraction> {
        let _span = tracing::trace_span!("view", ?window).entered();
        let blank = || Space::new().width(Length::Fill).height(Length::Fill);
        if let Some(&position) = self.backdrops.get(&window) {
            return backdrop_view(self.edge_shadow_rendering, position);
        }
        match self.surfaces.get(&window) {
            Some(Surface {
                output,
                role: SurfaceRole::Panel,
                ..
            }) => self.monitor_layout(window, output),
            Some(Surface {
                output,
                role: SurfaceRole::NotificationToasts,
                ..
            }) => Element::from(
                self.notifications.anchored_toasts(
                    output,
                    &self.icon_catalog,
                    &self.config,
                    &self.theme,
                    ToastSurfaceStyle {
                        overlay_appearance: self
                            .indicator_groups
                            .overlay_settings_group
                            .layer
                            .appearance(),
                        panel_clearance: self.panel_clearance(output),
                        background_fade: self.toast_background_fade(output),
                    },
                    self.clock,
                ),
            )
            .map(ShellInteraction::Notification),
            _ => blank().into(),
        }
    }

    #[must_use]
    pub fn theme(&self, _window: Id) -> iced::Theme {
        self.theme.clone()
    }

    #[must_use]
    pub fn style(theme: &iced::Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: text_color(theme),
        }
    }
}

fn message_work_kind(message: &Message) -> WorkKind {
    match message {
        Message::Interaction(ShellInteraction::Group(group_interaction)) => WorkKind {
            layer: "GroupInteraction",
            variant: group_interaction.into(),
        },
        Message::Interaction(shell_interaction) => WorkKind {
            layer: "ShellInteraction",
            variant: shell_interaction.into(),
        },
        message => WorkKind {
            layer: "Message",
            variant: message.into(),
        },
    }
}
