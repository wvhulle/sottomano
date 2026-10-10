pub mod app_window_focus;
pub mod bottom_bar;
pub mod hover_tooltips;
pub mod monitor_layout;
pub mod monitor_surfaces;
pub mod notification_toasts;
pub mod panel_bar;
pub mod top_bar;
pub mod workspace_sidebar;

use std::{
    collections::{BTreeMap, HashMap},
    mem,
    sync::Arc,
};

use chrono::Utc;
use iced::{
    Color, Element, Font, Length, Task, font,
    theme::Style,
    widget::{Space, container},
    window::Id,
};
use iced_exwlshell::to_layer_message;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};

use crate::{
    config::{
        Config, ConfigFile, LoadError, SecretPaths, bar_size::BarSize, font::ResolvedFont,
        temperature::SensorConfig,
    },
    data::{
        sink::sound,
        source::{
            app_icon::{self, DesktopEntries},
            command::blocking::run,
            desktop_entries::{self, DesktopIndex, ResolvedIcons},
            favicons::RawFavicon,
            launch_tracker::{Launch, LaunchTracker},
            niri,
        },
    },
    desktop_shell::{
        app_window_focus::AppWindowFocus,
        hover_tooltips::{TooltipHost, TooltipMessage},
        monitor_surfaces::{OutputChange, Position, Surface, SurfaceRole, query_outputs},
        notification_toasts::{NotificationLayer, NotificationMessage, ToastContext},
    },
    message_subscriptions::{blocking, copy_to_clipboard, niri_action},
    state::{
        self, ShellSettings,
        niri_rules::{NiriRulesFile, StoredRules},
    },
    widget_groups::{
        ai_group::{AiGroup, AiMessage},
        audio_group::{AudioGroup, AudioMessage},
        clock_group::{ClockGroup, ClockMessage},
        color_temperature_group::{ColorTemperatureGroup, ColorTemperatureMessage},
        compute_group::{ComputeGroup, ComputeMessage},
        dark_mode_group::{DarkModeGroup, DarkModeMessage},
        disk_group::{DiskGroup, DiskMessage},
        gpu_group::{GpuGroup, GpuMessage},
        idle_timeouts_group::{IdleTimeoutsGroup, IdleTimeoutsMessage},
        internet_group::{InternetGroup, InternetMessage},
        journal_events_group::{JournalEventsGroup, JournalEventsMessage},
        media_group::{self, MediaGroup, MediaMessage},
        monitor_group::{MonitorGroup, MonitorMessage},
        net_interfaces_group::{NetInterfacesGroup, NetInterfacesMessage},
        niri_window_group::{NiriWindowGroup, NiriWindowMessage},
        notification_group::{NotificationGroup, NotificationGroupMessage},
        overlay_group::{self, OverlayGroup, OverlayMessage},
        panel_appearance_group::{PanelAppearanceGroup, PanelAppearanceMessage},
        pending_upgrades_group::{PendingUpgradesGroup, PendingUpgradesMessage},
        power_group::{PowerGroup, PowerMessage},
        shell_layout_group::{self, ShellLayoutGroup, ShellLayoutMessage},
        surface_effects_group::{SurfaceEffectsGroup, SurfaceEffectsMessage},
        systemd_manager_group::{SystemdManagerGroup, SystemdManagerMessage},
        tray_group::{self, TrayGroup, TrayMessage},
        wallpaper_group::{WallpaperGroup, WallpaperMessage},
        weather_group::{WeatherGroup, WeatherMessage},
        window_group::{self, WindowGroup, WindowGroupMessage},
    },
    widgets::{
        favicon::Favicons,
        text::FontFallbacks,
        widget_group::{GroupEvent, GroupExpansion},
    },
};

pub struct DesktopShell {
    pub(crate) config_file: ConfigFile,
    pub(crate) config: Config,
    pub(crate) theme: iced::Theme,
    pub(crate) shell_settings: ShellSettings,
    pub(crate) weather_group: WeatherGroup,
    pub(crate) wallpaper_group: WallpaperGroup,
    pub(crate) power_group: PowerGroup,
    pub(crate) compute_group: ComputeGroup,
    pub(crate) gpu_group: GpuGroup,
    pub(crate) disk_group: DiskGroup,
    pub(crate) niri_rules: NiriRulesFile,
    pub(crate) shell_layout_group: ShellLayoutGroup,
    pub(crate) panel_appearance_group: PanelAppearanceGroup,
    pub(crate) overlay_group: OverlayGroup,
    pub(crate) surface_effects_group: SurfaceEffectsGroup,
    pub(crate) niri_window_group: NiriWindowGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) notification_group: NotificationGroup,
    pub(crate) systemd_manager_group: SystemdManagerGroup,
    pub(crate) journal_events_group: JournalEventsGroup,
    pub(crate) pending_upgrades_group: PendingUpgradesGroup,
    pub(crate) internet_group: InternetGroup,
    pub(crate) net_interfaces_group: NetInterfacesGroup,
    pub(crate) monitor_group: MonitorGroup,
    pub(crate) color_temperature_group: ColorTemperatureGroup,
    pub(crate) dark_mode_group: DarkModeGroup,
    pub(crate) idle_timeouts_group: IdleTimeoutsGroup,
    pub(crate) media_group: MediaGroup,
    pub(crate) audio_group: AudioGroup,
    pub(crate) ai_group: AiGroup,
    pub(crate) tray_group: TrayGroup,
    pub(crate) window_group: WindowGroup,
    pub(crate) launch_tracker: LaunchTracker,
    pub(crate) niri: EventStreamState,
    pub(crate) niri_snapshot: niri::Snapshot,
    pub(crate) outputs: HashMap<String, niri::OutputInfo>,
    pub(crate) surfaces: BTreeMap<Id, Surface>,
    pub(crate) backdrops: BTreeMap<Id, Position>,
    pub(crate) sidebar_widths: HashMap<Id, f32>,
    pub(crate) tooltip: TooltipHost,
    pub(crate) notifications: NotificationLayer,
    pub(crate) desktop_entries: DesktopIndex,
    pub(crate) favicons: Favicons,
    pub(crate) font_fallbacks: FontFallbacks,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Weather(WeatherMessage),
    Wallpaper(WallpaperMessage),
    Power(PowerMessage),
    Compute(ComputeMessage),
    Gpu(GpuMessage),
    Disk(DiskMessage),
    ShellLayout(ShellLayoutMessage),
    PanelAppearance(PanelAppearanceMessage),
    Overlay(OverlayMessage),
    SurfaceEffects(SurfaceEffectsMessage),
    NiriWindow(NiriWindowMessage),
    Clock(ClockMessage),
    NotificationGroup(NotificationGroupMessage),
    SystemdManager(SystemdManagerMessage),
    JournalEvents(JournalEventsMessage),
    PendingUpgrades(PendingUpgradesMessage),
    Internet(InternetMessage),
    NetInterfaces(NetInterfacesMessage),
    Monitor(MonitorMessage),
    ColorTemperature(ColorTemperatureMessage),
    DarkMode(DarkModeMessage),
    IdleTimeouts(IdleTimeoutsMessage),
    Media(MediaMessage),
    Tray(TrayMessage),
    ShellSettingsLoaded(ShellSettings),
    NiriRulesLoaded(StoredRules),
    ConfigFileChanged,
    ConfigLoaded(Result<Box<Config>, Arc<LoadError>>),
    SoundThemeLoaded(Option<String>),
    Audio(AudioMessage),
    Ai(AiMessage),
    WindowGroup(WindowGroupMessage),
    TrackerLoaded(Vec<Launch>),
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    IconsResolved(ResolvedIcons),
    FaviconsResolved(HashMap<String, RawFavicon>),
    FallbackFontsResolved(HashMap<char, Font>),
    DismissPressed,
    WorkspaceClicked { id: u64, is_active: bool },
    Niri(Vec<niri_ipc::Event>),
    Redraw,
    WindowClosed(Id),
    PointerLeft,
    CopyText(String),
    OpenUrl(String),
    FocusWindow(u64),
    TooltipTick,
    TooltipHideElapsed,
    Tooltip(TooltipMessage),
    Notification(NotificationMessage),
    OutputsResolved(HashMap<String, niri::OutputInfo>),
    Output(OutputChange),
    ReloadDesktopEntries,
    SidebarWidthChanged { window: Id, width: f32 },
}

impl From<TooltipMessage> for Message {
    fn from(message: TooltipMessage) -> Self {
        Message::Tooltip(message)
    }
}

impl From<GroupEvent<Message>> for Message {
    fn from(event: GroupEvent<Message>) -> Self {
        match event {
            GroupEvent::Group(message) => message,
            GroupEvent::Tooltip(tooltip) => Message::Tooltip(tooltip),
        }
    }
}

impl DesktopShell {
    pub fn new(
        config: Config,
        config_file: ConfigFile,
        secrets: SecretPaths,
        niri_rules: NiriRulesFile,
        loaded: Result<Box<Config>, Arc<LoadError>>,
    ) -> (Self, Task<Message>) {
        let _span = tracing::debug_span!("desktop shell init").entered();
        let shell = Self {
            weather_group: WeatherGroup::default(),
            wallpaper_group: WallpaperGroup::new(&secrets),
            power_group: PowerGroup::default(),
            compute_group: ComputeGroup::default(),
            gpu_group: GpuGroup::default(),
            disk_group: DiskGroup::default(),
            niri_rules,
            shell_layout_group: ShellLayoutGroup::default(),
            panel_appearance_group: PanelAppearanceGroup::default(),
            overlay_group: OverlayGroup::default(),
            surface_effects_group: SurfaceEffectsGroup::default(),
            niri_window_group: NiriWindowGroup::default(),
            clock_group: ClockGroup::default(),
            notification_group: NotificationGroup::default(),
            systemd_manager_group: SystemdManagerGroup::default(),
            journal_events_group: JournalEventsGroup::default(),
            pending_upgrades_group: PendingUpgradesGroup::default(),
            internet_group: InternetGroup::default(),
            net_interfaces_group: NetInterfacesGroup::default(),
            monitor_group: MonitorGroup::default(),
            color_temperature_group: ColorTemperatureGroup::new(
                config.color_temperature_service.clone(),
            ),
            dark_mode_group: DarkModeGroup::default(),
            idle_timeouts_group: IdleTimeoutsGroup::default(),
            media_group: MediaGroup::default(),
            audio_group: AudioGroup::default(),
            ai_group: AiGroup::new(secrets),
            tray_group: TrayGroup::default(),
            window_group: WindowGroup::default(),
            launch_tracker: LaunchTracker::default(),
            niri: EventStreamState::default(),
            niri_snapshot: niri::Snapshot::default(),
            outputs: HashMap::new(),
            surfaces: BTreeMap::new(),
            backdrops: BTreeMap::new(),
            sidebar_widths: HashMap::new(),
            tooltip: TooltipHost::default(),
            notifications: NotificationLayer::default(),
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            font_fallbacks: FontFallbacks::default(),
            shell_settings: ShellSettings::default(),
            theme: iced::Theme::from(&config.palette),
            config,
            config_file,
        };
        let init = Task::batch([
            Task::done(Message::ConfigLoaded(loaded)),
            desktop_entries::load().map(Message::DesktopEntriesLoaded),
            Task::perform(state::launches::load(), Message::TrackerLoaded),
            Task::perform(state::idle::load(), |t| {
                Message::IdleTimeouts(IdleTimeoutsMessage::Loaded(t))
            }),
            Task::perform(state::shell_settings::load(), Message::ShellSettingsLoaded),
            shell.niri_rules.read().map(Message::NiriRulesLoaded),
            blocking("sound theme", sound::session_theme).map(Message::SoundThemeLoaded),
            query_outputs(),
            PowerGroup::probe().map(Message::Power),
            ComputeGroup::probe(&shell.config.temperature.cpu).map(Message::Compute),
            GpuGroup::probe().map(Message::Gpu),
            InternetGroup::probe().map(Message::Internet),
            MonitorGroup::probe().map(Message::Monitor),
            DarkModeGroup::probe().map(Message::DarkMode),
            shell
                .color_temperature_group
                .probe()
                .map(Message::ColorTemperature),
            shell.ai_group.probe().map(Message::Ai),
            shell.wallpaper_group.probe().map(Message::Wallpaper),
        ]);
        (shell, init)
    }

    pub fn namespace() -> String {
        "vetrata".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let _span = tracing::trace_span!("update", ?message).entered();
        let tooltip_seq = self.tooltip.seq();
        let dispatched = self.dispatch_message(message);
        let requests = self.group_toast_requests();
        let widget_toasts = self.notifications.show_widget_toasts(
            requests,
            &ToastContext::new(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                &self.shell_settings,
            ),
        );
        Task::batch([dispatched, widget_toasts, self.redraw_content(tooltip_seq)])
    }

    fn redraw_content(&self, tooltip_seq_before_update: u64) -> Task<Message> {
        Task::batch(
            self.panels()
                .map(|(panel, _)| panel)
                .chain(self.tooltip.surfaces_to_redraw(tooltip_seq_before_update))
                .map(|surface| Task::done(Message::RedrawWindow(surface))),
        )
    }

    fn dispatch_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Output(change) => self.apply_output_change(change),
            Message::WindowGroup(msg) => self.update_window_group(msg),
            Message::TrackerLoaded(launches) => self.load_launch_tracker(launches),
            Message::DesktopEntriesLoaded(entries) => self.load_desktop_entries(entries),
            Message::ReloadDesktopEntries => {
                desktop_entries::load().map(Message::DesktopEntriesLoaded)
            }
            Message::IconsResolved(resolved) => {
                self.desktop_entries.merge(resolved);
                Task::none()
            }
            Message::FaviconsResolved(raw) => {
                self.favicons.merge(raw);
                Task::none()
            }
            Message::FallbackFontsResolved(fonts) => {
                self.font_fallbacks.merge(fonts);
                Task::none()
            }
            Message::Weather(msg) => self.weather_group.update(msg).map(Message::Weather),
            Message::Wallpaper(msg) => self.wallpaper_group.update(msg).map(Message::Wallpaper),
            Message::Notification(msg) => self.update_notification(msg),
            Message::Power(msg) => self.power_group.update(msg).map(Message::Power),
            Message::Compute(msg) => self.compute_group.update(msg).map(Message::Compute),
            Message::Gpu(msg) => self.gpu_group.update(msg).map(Message::Gpu),
            Message::Disk(msg) => self.disk_group.update(msg).map(Message::Disk),
            Message::ShellLayout(msg) => self.update_shell_layout(msg),
            Message::PanelAppearance(msg) => self.update_panel_appearance(msg),
            Message::Overlay(msg) => self.update_overlay(msg),
            Message::SurfaceEffects(msg) => self.update_surface_effects(msg),
            Message::NiriWindow(msg) => self.niri_window_group.update(msg, &mut self.niri_rules),
            Message::NiriRulesLoaded(stored) => self.adopt_niri_rules(stored),
            Message::Clock(msg) => self.update_clock(msg),
            Message::NotificationGroup(msg) => self.update_notification_group(msg),
            Message::SystemdManager(msg) => self.update_systemd_manager(msg),
            Message::JournalEvents(msg) => self.update_journal_events(msg),
            Message::PendingUpgrades(msg) => self.update_pending_upgrades(msg),
            Message::NetInterfaces(msg) => self.update_net_interfaces(msg),
            Message::Internet(msg) => self.internet_group.update(msg).map(Message::Internet),
            Message::Monitor(msg) => self.monitor_group.update(msg).map(Message::Monitor),
            Message::ColorTemperature(msg) => self.update_color_temperature(msg),
            Message::DarkMode(msg) => self.dark_mode_group.update(msg).map(Message::DarkMode),
            Message::Media(msg) => self.update_media(msg),
            Message::Tray(msg) => self.update_tray(msg),
            Message::IdleTimeouts(msg) => self.update_idle_timeouts(msg),
            Message::SoundThemeLoaded(theme) => self.adopt_sound_theme(theme),
            Message::ShellSettingsLoaded(shell_settings) => {
                self.apply_saved_shell_settings(shell_settings)
            }
            Message::ConfigFileChanged => self.reload_config(),
            Message::ConfigLoaded(Ok(config)) => self.adopt_config(*config),
            Message::ConfigLoaded(Err(error)) => self.reject_config(&error),
            Message::Audio(msg) => self.audio_group.update(msg).map(Message::Audio),
            Message::Ai(msg) => self.ai_group.update(msg).map(Message::Ai),
            Message::DismissPressed => self.dismiss_groups(),
            Message::WorkspaceClicked {
                is_active: true, ..
            } => niri_action(niri_ipc::Action::ToggleOverview {}),
            Message::WorkspaceClicked { id, .. } => niri_action(niri_ipc::Action::FocusWorkspace {
                reference: niri_ipc::WorkspaceReferenceArg::Id(id),
            }),
            Message::Niri(events) => self.apply_niri_events(events),
            Message::WindowClosed(window) => self.forget_window(window),
            Message::SidebarWidthChanged { window, width } => {
                self.update_sidebar_width(window, width)
            }
            Message::Tooltip(msg) => self.update_tooltip(msg),
            Message::PointerLeft => self.tooltip.leave_after_grace(),
            Message::CopyText(text) => copy_to_clipboard(text),
            Message::OpenUrl(url) => self.open_url(url),
            Message::FocusWindow(window) => Task::batch([
                self.dismiss_tooltip(),
                niri_action(niri_ipc::Action::FocusWindow { id: window }),
            ]),
            Message::TooltipHideElapsed => self.leave_tooltip_after_grace(),
            Message::OutputsResolved(outputs) => self.adopt_outputs(outputs),
            Message::Redraw => self.prune_notifications(),
            Message::TooltipTick
            | Message::LayoutChange { .. }
            | Message::BlurOptionChange { .. }
            | Message::PopUpReposition { .. }
            | Message::SetInputRegion { .. }
            | Message::RequestActivationToken { .. }
            | Message::LayerChange { .. }
            | Message::MarginChange { .. }
            | Message::ExclusiveZoneChange { .. }
            | Message::KeyboardInteractivityChange { .. }
            | Message::VirtualKeyboardPressed { .. }
            | Message::NewLayerShell { .. }
            | Message::NewBaseWindow { .. }
            | Message::NewPopUp { .. }
            | Message::NewMenu { .. }
            | Message::NewInputPanel { .. }
            | Message::RemoveWindow(_)
            | Message::RedrawWindow(_)
            | Message::ForgetLastOutput => Task::none(),
        }
    }

    fn open_url(&mut self, url: String) -> Task<Message> {
        Task::batch([
            self.dismiss_tooltip(),
            blocking("open url", move || run("xdg-open", &[&url])).discard(),
        ])
    }

    fn update_systemd_manager(&mut self, msg: SystemdManagerMessage) -> Task<Message> {
        self.systemd_manager_group
            .update(msg)
            .map(Message::SystemdManager)
    }

    fn update_pending_upgrades(&mut self, msg: PendingUpgradesMessage) -> Task<Message> {
        self.pending_upgrades_group
            .update(msg)
            .map(Message::PendingUpgrades)
    }

    fn update_journal_events(&mut self, msg: JournalEventsMessage) -> Task<Message> {
        self.journal_events_group
            .update(msg)
            .map(Message::JournalEvents)
    }

    fn update_clock(&mut self, msg: ClockMessage) -> Task<Message> {
        self.clock_group
            .update(msg, &self.config)
            .map(Message::Clock)
    }

    fn update_net_interfaces(&mut self, msg: NetInterfacesMessage) -> Task<Message> {
        self.net_interfaces_group
            .update(msg)
            .map(Message::NetInterfaces)
    }

    fn update_panel_appearance(&mut self, msg: PanelAppearanceMessage) -> Task<Message> {
        self.panel_appearance_group
            .update(msg, &mut self.shell_settings.panels)
    }

    fn update_surface_effects(&mut self, msg: SurfaceEffectsMessage) -> Task<Message> {
        self.surface_effects_group.update(
            msg,
            &mut self.shell_settings.shadow,
            &mut self.niri_rules,
        )
    }

    fn update_overlay(&mut self, msg: OverlayMessage) -> Task<Message> {
        match self
            .overlay_group
            .update(msg, &mut self.shell_settings.overlays)
        {
            overlay_group::Action::None => Task::none(),
            overlay_group::Action::Run(task) => task,
            overlay_group::Action::ReflowToasts(task) => {
                let placement = niri::Placement::derive(&self.niri);
                Task::batch([
                    self.notifications.on_niri_changed(
                        &placement,
                        &self.config,
                        self.shell_settings.overlays,
                    ),
                    task,
                ])
            }
        }
    }

    fn dismiss_groups(&mut self) -> Task<Message> {
        self.collapse_groups();
        Task::none()
    }

    fn update_idle_timeouts(&mut self, msg: IdleTimeoutsMessage) -> Task<Message> {
        self.idle_timeouts_group
            .update(msg)
            .map(Message::IdleTimeouts)
    }

    fn update_color_temperature(&mut self, msg: ColorTemperatureMessage) -> Task<Message> {
        self.color_temperature_group
            .update(msg)
            .map(Message::ColorTemperature)
    }

    fn adopt_sound_theme(&mut self, theme: Option<String>) -> Task<Message> {
        self.notifications.adopt_sound_theme(theme);
        Task::none()
    }

    fn load_launch_tracker(&mut self, launches: Vec<Launch>) -> Task<Message> {
        self.launch_tracker.load_from(launches);
        self.refresh_quick_launch()
    }

    fn load_desktop_entries(&mut self, entries: Arc<DesktopEntries>) -> Task<Message> {
        self.desktop_entries.set(entries.clone());
        self.window_group.set_desktop_entries(entries);
        self.refresh_quick_launch()
    }

    fn adopt_outputs(&mut self, outputs: HashMap<String, niri::OutputInfo>) -> Task<Message> {
        self.outputs = outputs;
        let placement = niri::Placement::derive(&self.niri);
        self.notifications
            .on_niri_changed(&placement, &self.config, self.shell_settings.overlays)
    }

    fn update_window_group(&mut self, msg: WindowGroupMessage) -> Task<Message> {
        match self.window_group.update(msg) {
            window_group::Action::Run(task) => task.map(Message::WindowGroup),
            window_group::Action::FocusWindow(window) => {
                niri_action(niri_ipc::Action::FocusWindow { id: window })
            }
            window_group::Action::MaximizeColumn => {
                niri_action(niri_ipc::Action::MaximizeColumn {})
            }
            window_group::Action::ToggleWindowAudio(pid) => {
                self.audio_group.toggle_window_mute(pid).map(Message::Audio)
            }
            window_group::Action::RefreshWindowList => Task::batch([
                self.favicons
                    .retry_unresolved()
                    .map(Message::FaviconsResolved),
                self.resolve_icons(),
            ]),
            window_group::Action::None => Task::none(),
        }
    }

    fn update_media(&mut self, msg: MediaMessage) -> Task<Message> {
        let players_changed = matches!(msg, MediaMessage::Reading(_));
        let task = match self.media_group.update(msg) {
            media_group::Action::Run(task) => task.map(Message::Media),
            media_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Media)
            }
        };
        if players_changed {
            Task::batch([task, self.resolve_icons()])
        } else {
            task
        }
    }

    fn update_tray(&mut self, msg: TrayMessage) -> Task<Message> {
        match self.tray_group.update(msg) {
            tray_group::Action::None => Task::none(),
            tray_group::Action::MenuItemActivated => self.dismiss_tooltip(),
            tray_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Tray)
            }
        }
    }

    fn focus_app_window<M: Send + 'static>(
        &self,
        focus: AppWindowFocus<M>,
        lift: fn(M) -> Message,
    ) -> Task<Message> {
        let placement = niri::Placement::derive(&self.niri);
        let Some(window) = placement.window_owned_by(&focus.owner) else {
            tracing::debug!("no single window owned by {:?}", focus.owner);
            return Task::done(lift(focus.unmatched));
        };
        niri_action(niri_ipc::Action::FocusWindow { id: window })
    }

    fn apply_saved_shell_settings(&mut self, shell_settings: ShellSettings) -> Task<Message> {
        self.shell_settings = shell_settings;
        self.shell_layout_group.adopt(&self.shell_settings.layout);
        self.panel_appearance_group
            .adopt(&self.shell_settings.panels);
        self.overlay_group.adopt(self.shell_settings.overlays);
        self.surface_effects_group.adopt(self.shell_settings.shadow);
        self.reserve()
    }

    fn adopt_niri_rules(&mut self, stored: StoredRules) -> Task<Message> {
        let written = self.niri_rules.adopt(stored);
        self.niri_window_group
            .adopt(self.niri_rules.styling().layout);
        written.discard()
    }

    fn reload_config(&self) -> Task<Message> {
        let file = self.config_file.clone();
        blocking("config reload", move || Some(file.load())).and_then(|loaded| {
            Task::done(Message::ConfigLoaded(
                loaded.map(Box::new).map_err(Arc::new),
            ))
        })
    }

    fn adopt_config(&mut self, config: Config) -> Task<Message> {
        let previous = mem::replace(&mut self.config, config);
        self.theme = iced::Theme::from(&self.config.palette);
        Task::batch([
            self.notifications.withdraw_config_error(),
            self.adopt_font(&previous.font),
            self.adopt_bar_size(&previous.bar),
            self.adopt_color_temperature_service(&previous.color_temperature_service),
            self.adopt_cpu_temperature_sensor(&previous.temperature.cpu),
        ])
    }

    fn adopt_cpu_temperature_sensor(&self, previous: &SensorConfig) -> Task<Message> {
        if &self.config.temperature.cpu == previous {
            return Task::none();
        }
        ComputeGroup::probe(&self.config.temperature.cpu).map(Message::Compute)
    }

    fn adopt_font(&mut self, previous: &ResolvedFont) -> Task<Message> {
        if self.config.font.has_same_face_as(previous) {
            return Task::none();
        }
        self.font_fallbacks = FontFallbacks::default();
        font::load(self.config.font.data.to_vec()).map(|loaded| {
            if let Err(e) = loaded {
                tracing::warn!("config reload: font not loaded: {e:?}");
            }
            Message::Redraw
        })
    }

    fn adopt_bar_size(&mut self, previous: &BarSize) -> Task<Message> {
        if self.config.bar == *previous {
            return Task::none();
        }
        Task::batch([self.resize_backdrops(), self.reserve()])
    }

    fn adopt_color_temperature_service(&mut self, previous: &str) -> Task<Message> {
        if self.config.color_temperature_service == previous {
            return Task::none();
        }
        self.color_temperature_group =
            ColorTemperatureGroup::new(self.config.color_temperature_service.clone());
        self.color_temperature_group
            .probe()
            .map(Message::ColorTemperature)
    }

    fn reject_config(&mut self, error: &LoadError) -> Task<Message> {
        let path = self.config_file.path();
        tracing::warn!("invalid configuration {}: {error}", path.display());
        self.notifications.show_config_error(
            path,
            error,
            &ToastContext::new(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                &self.shell_settings,
            ),
        )
    }

    fn collapse_groups(&mut self) {
        if !self.shell_settings.layout.auto_collapse_groups {
            return;
        }
        self.shell_layout_group.collapse();
        self.panel_appearance_group.collapse();
        self.overlay_group.collapse();
        self.surface_effects_group.collapse();
        self.niri_window_group.collapse();
        self.monitor_group.collapse();
        self.color_temperature_group.collapse();
        self.dark_mode_group.collapse();
        self.power_group.collapse();
        self.idle_timeouts_group.collapse();
        self.audio_group.collapse();
        let _ = [
            &mut self.wallpaper_group.expansion,
            &mut self.clock_group.expansion,
            &mut self.weather_group.expansion,
            &mut self.compute_group.expansion,
            &mut self.gpu_group.expansion,
            &mut self.disk_group.expansion,
            &mut self.media_group.expansion,
            &mut self.internet_group.expansion,
            &mut self.net_interfaces_group.expansion,
            &mut self.window_group.quick_launch.expansion,
            &mut self.tray_group.expansion,
            &mut self.ai_group.expansion,
            &mut self.notification_group.expansion,
            &mut self.systemd_manager_group.expansion,
            &mut self.journal_events_group.expansion,
            &mut self.pending_upgrades_group.expansion,
        ]
        .map(GroupExpansion::collapse);
    }

    fn apply_niri_events(&mut self, events: Vec<niri_ipc::Event>) -> Task<Message> {
        let mut window_change = false;
        let mut workspace_change = false;
        let mut focus_change = false;
        let mut tasks = Vec::new();
        for event in events {
            window_change |= matches!(
                event,
                niri_ipc::Event::WindowsChanged { .. }
                    | niri_ipc::Event::WindowOpenedOrChanged { .. }
                    | niri_ipc::Event::WindowClosed { .. }
            );
            workspace_change |= matches!(
                event,
                niri_ipc::Event::WorkspacesChanged { .. }
                    | niri_ipc::Event::WorkspaceActivated { .. }
            );
            focus_change |= matches!(event, niri_ipc::Event::WindowFocusChanged { .. });
            if let Some(launch) = self.launch_tracker.observe(&event) {
                tasks.push(Task::future(state::launches::record(launch)).discard());
            }
            let _ = self.niri.apply(event);
        }
        self.niri_snapshot = niri::Snapshot::derive(&self.niri);
        if focus_change {
            self.collapse_groups();
        }
        if window_change {
            tasks.push(self.refresh_window_assets());
        }
        if window_change || workspace_change {
            tasks.push(self.refresh_quick_launch());
        }
        if self.notifications.is_active() {
            let placement = niri::Placement::derive(&self.niri);
            tasks.push(self.notifications.on_niri_changed(
                &placement,
                &self.config,
                self.shell_settings.overlays,
            ));
        }
        Task::batch(tasks)
    }

    fn refresh_quick_launch(&mut self) -> Task<Message> {
        let Some(entries) = self.desktop_entries.arc() else {
            return Task::none();
        };
        let now = Utc::now();
        let ranked = self
            .niri_snapshot
            .active_workspaces()
            .filter_map(|active| {
                let workspace = active.name?;
                let open = self.niri_snapshot.open_app_ids(active.output);
                let apps = self.launch_tracker.rank(workspace, &open, now, |app_id| {
                    entries.is_launchable(app_id)
                });
                (!apps.is_empty()).then(|| (active.output.to_owned(), apps))
            })
            .collect();
        self.window_group.set_ranked(ranked);
        self.resolve_icons()
    }

    fn refresh_window_assets(&mut self) -> Task<Message> {
        let titles = niri::all_windows(&self.niri);
        Task::batch([
            self.favicons
                .track_titles(&self.niri)
                .map(Message::FaviconsResolved),
            self.font_fallbacks
                .resolve_missing(
                    titles
                        .iter()
                        .filter_map(|win| win.title.as_deref())
                        .flat_map(str::chars),
                    &self.config.font,
                )
                .map(Message::FallbackFontsResolved),
        ])
    }

    fn resolve_icons(&mut self) -> Task<Message> {
        let mut app_keys = app_icon::app_icon_keys(&self.niri);
        app_keys.extend(self.window_group.icon_keys());
        app_keys.extend(self.media_group.icon_keys());
        let theme_keys = app_icon::theme_icon_keys(&self.niri);
        self.desktop_entries
            .resolve_missing(app_keys, theme_keys)
            .map(Message::IconsResolved)
    }

    fn update_shell_layout(&mut self, msg: ShellLayoutMessage) -> Task<Message> {
        match self
            .shell_layout_group
            .update(msg, &mut self.shell_settings.layout)
        {
            shell_layout_group::Action::None => Task::none(),
            shell_layout_group::Action::Run(task) => task,
            shell_layout_group::Action::Reflow(task) => Task::batch([self.reserve(), task]),
        }
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        let _span = tracing::trace_span!("view", ?window).entered();
        if let Some(tooltip) = self.tooltip.view(
            window,
            &self.config,
            &self.shell_settings,
            &self.favicons,
            |target| self.tooltip_for(target),
        ) {
            return tooltip;
        }
        let blank = || Space::new().width(Length::Fill).height(Length::Fill);
        if self.backdrops.contains_key(&window) {
            return container(blank())
                .style(|_| container::background(Color::BLACK))
                .into();
        }
        match self.surfaces.get(&window) {
            Some(Surface {
                output,
                role: SurfaceRole::Panel,
            }) => self.monitor_layout(window, output),
            _ => blank().into(),
        }
    }

    pub fn theme(&self, _window: Id) -> iced::Theme {
        self.theme.clone()
    }

    pub fn style(&self, _theme: &iced::Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: self.config.palette.bar_foreground,
        }
    }
}
