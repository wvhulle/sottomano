use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use chrono::Utc;
use iced::{Color, Element, Font, Task, theme::Style, window::Id};
use iced_layershell::{actions::LayerShellOutputEvent, to_layer_message};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};

use crate::{
    config::Config,
    data::{
        sink::sound,
        source::{
            Tooltip, TooltipBody,
            app_icon::{self, DesktopEntries},
            desktop_entries::{self, DesktopIndex, ResolvedIcons},
            favicons::RawFavicon,
            launch_tracker::{Buckets, LaunchTracker},
            niri,
        },
    },
    message_subscriptions::{blocking, niri_action},
    state::{self, Session},
    wayland_layers::{
        backdrop_shadow,
        bar::{float_margin, layer_settings, view},
        blur_caster::{BlurCasters, Zone},
        notifications::{NotificationLayer, NotificationMessage},
        surface::{Margin, Position, Surface},
        tooltip::{HoverTarget, TooltipHost, TooltipMessage, tooltip_transition::Departed},
    },
    widget_groups::{
        ai_group::{AiGroup, AiMessage},
        audio_group::{AudioGroup, AudioMessage},
        battery_group::{BatteryGroup, BatteryMessage},
        clock_group::{ClockGroup, ClockMessage},
        color_temperature_group::{ColorTemperatureGroup, ColorTemperatureMessage},
        idle_timeouts_group::{IdleTimeoutsGroup, IdleTimeoutsMessage},
        internet_group::{InternetGroup, InternetMessage},
        journal_group::{JournalGroup, JournalMessage},
        media_group::{MediaGroup, MediaMessage},
        monitor_group::{MonitorGroup, MonitorMessage},
        net_interfaces_group::{NetInterfacesGroup, NetInterfacesMessage},
        notification_group::NotificationGroupMessage,
        output_group,
        panel_settings_group::{self, PanelSettingsGroup, PanelSettingsMessage},
        power_profile_group::{PowerProfileGroup, PowerProfileMessage},
        system_group::{SystemGroup, SystemMessage},
        tray_group::{self, TrayGroup, TrayMessage},
        weather_group::{WeatherGroup, WeatherMessage},
        window_group::{
            self, WindowGroup, WindowGroupMessage, quick_launch::MAX_ITEMS, window_tooltip,
        },
        workspaces_group::workspace_tooltip,
    },
    widgets::{favicon::Favicons, menu_group::MenuGroup, text::FontFallbacks},
};

pub struct Bar {
    pub(crate) config: Config,
    pub(crate) session: Session,
    pub(crate) weather_group: WeatherGroup,
    pub(crate) battery_group: BatteryGroup,
    pub(crate) system_group: SystemGroup,
    pub(crate) panel_settings_group: PanelSettingsGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) journal_group: JournalGroup,
    pub(crate) internet_group: InternetGroup,
    pub(crate) net_interfaces_group: NetInterfacesGroup,
    pub(crate) monitor_group: MonitorGroup,
    pub(crate) color_temperature_group: ColorTemperatureGroup,
    pub(crate) idle_timeouts_group: IdleTimeoutsGroup,
    pub(crate) media_group: MediaGroup,
    pub(crate) audio_group: AudioGroup,
    pub(crate) power_profile_group: PowerProfileGroup,
    pub(crate) ai_group: AiGroup,
    pub(crate) tray_group: TrayGroup,
    pub(crate) window_group: WindowGroup,
    pub(crate) launch_tracker: LaunchTracker,
    pub(crate) niri: EventStreamState,
    pub(crate) niri_snapshot: niri::Snapshot,
    pub(crate) outputs: HashMap<String, niri::OutputInfo>,
    pub(crate) surfaces: BTreeMap<Id, Surface>,
    pub(crate) backdrops: HashSet<Id>,
    pub(crate) blur_casters: BlurCasters,
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
    Battery(BatteryMessage),
    System(SystemMessage),
    PanelSettings(PanelSettingsMessage),
    Clock(ClockMessage),
    NotificationGroup(NotificationGroupMessage),
    Journal(JournalMessage),
    Internet(InternetMessage),
    NetInterfaces(NetInterfacesMessage),
    Monitor(MonitorMessage),
    ColorTemperature(ColorTemperatureMessage),
    IdleTimeouts(IdleTimeoutsMessage),
    Media(MediaMessage),
    Tray(TrayMessage),
    PanelLoaded(Session),
    SoundThemeLoaded(Option<String>),
    Audio(AudioMessage),
    PowerProfile(PowerProfileMessage),
    Ai(AiMessage),
    WindowGroup(WindowGroupMessage),
    TrackerLoaded(Buckets),
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    IconsResolved(ResolvedIcons),
    FaviconsResolved(HashMap<String, RawFavicon>),
    FallbackFontsResolved(HashMap<char, Font>),
    DismissPressed,
    WorkspaceClicked {
        id: u64,
        is_active: bool,
    },
    OverviewToggled,
    Niri(Vec<niri_ipc::Event>),
    Redraw,
    WindowClosed(Id),
    PointerLeft,
    TooltipTick,
    TooltipHideElapsed,
    Tooltip(TooltipMessage),
    Notification(NotificationMessage),
    OutputsResolved(HashMap<String, niri::OutputInfo>),
    ReloadDesktopEntries,
    PillBounds {
        window: Id,
        zone: Zone,
        rects: Vec<iced::Rectangle>,
    },
}

impl Bar {
    pub fn new(config: Config) -> (Self, Task<Message>) {
        let bar = Self {
            weather_group: WeatherGroup::default(),
            battery_group: BatteryGroup::default(),
            system_group: SystemGroup::default(),
            panel_settings_group: PanelSettingsGroup::default(),
            clock_group: ClockGroup::default(),
            journal_group: JournalGroup::default(),
            internet_group: InternetGroup::default(),
            net_interfaces_group: NetInterfacesGroup::default(),
            monitor_group: MonitorGroup::default(),
            color_temperature_group: ColorTemperatureGroup::new(
                config.color_temperature_unit.clone(),
            ),
            idle_timeouts_group: IdleTimeoutsGroup::default(),
            media_group: MediaGroup::default(),
            audio_group: AudioGroup::default(),
            power_profile_group: PowerProfileGroup::default(),
            ai_group: AiGroup::new(config.secrets.clone()),
            tray_group: TrayGroup::default(),
            window_group: WindowGroup::default(),
            launch_tracker: LaunchTracker::default(),
            niri: EventStreamState::default(),
            niri_snapshot: niri::Snapshot::default(),
            outputs: HashMap::new(),
            surfaces: BTreeMap::new(),
            backdrops: HashSet::new(),
            blur_casters: BlurCasters::default(),
            tooltip: TooltipHost::default(),
            notifications: NotificationLayer::default(),
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            font_fallbacks: FontFallbacks::default(),
            session: Session::default(),
            config,
        };
        let init = Task::batch([
            desktop_entries::load().map(Message::DesktopEntriesLoaded),
            Task::perform(state::launches::load(), Message::TrackerLoaded),
            Task::perform(state::idle::load(), |t| {
                Message::IdleTimeouts(IdleTimeoutsMessage::Loaded(t))
            }),
            Task::perform(state::panel::load(), Message::PanelLoaded),
            blocking("sound theme", sound::session_theme).map(Message::SoundThemeLoaded),
            query_outputs(),
            BatteryGroup::probe().map(Message::Battery),
            SystemGroup::probe().map(Message::System),
            InternetGroup::probe().map(Message::Internet),
            PowerProfileGroup::probe().map(Message::PowerProfile),
            MonitorGroup::probe().map(Message::Monitor),
            bar.color_temperature_group
                .probe()
                .map(Message::ColorTemperature),
            bar.ai_group.probe().map(Message::Ai),
        ]);
        (bar, init)
    }

    pub fn namespace() -> String {
        "vetrata".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OutputEvent(event) => self.apply_output_event(event),
            Message::WindowGroup(msg) => self.update_window_group(msg),
            Message::TrackerLoaded(buckets) => self.load_launch_tracker(buckets),
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
                self.rederive_snapshot();
                Task::none()
            }
            Message::Weather(msg) => self.weather_group.update(msg).map(Message::Weather),
            Message::Battery(msg) => self.battery_group.update(msg).map(Message::Battery),
            Message::System(msg) => self.system_group.update(msg).map(Message::System),
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::Clock(msg) => self.clock_group.update(msg).map(Message::Clock),
            Message::NotificationGroup(msg) => self.update_notification_group(msg),
            Message::Journal(msg) => self.journal_group.update(msg).map(Message::Journal),
            Message::NetInterfaces(msg) => self
                .net_interfaces_group
                .update(msg)
                .map(Message::NetInterfaces),
            Message::Internet(msg) => self.internet_group.update(msg).map(Message::Internet),
            Message::Monitor(msg) => self.monitor_group.update(msg).map(Message::Monitor),
            Message::ColorTemperature(msg) => self.update_color_temperature(msg),
            Message::Media(msg) => self.update_media(msg),
            Message::Tray(msg) => self.update_tray(msg),
            Message::IdleTimeouts(msg) => self.update_idle_timeouts(msg),
            Message::SoundThemeLoaded(theme) => self.adopt_sound_theme(theme),
            Message::PanelLoaded(session) => self.apply_saved_panel_settings(session),
            Message::Audio(msg) => self.audio_group.update(msg).map(Message::Audio),
            Message::PowerProfile(msg) => self
                .power_profile_group
                .update(msg)
                .map(Message::PowerProfile),
            Message::Ai(msg) => self.ai_group.update(msg).map(Message::Ai),
            Message::DismissPressed => {
                self.dismiss_menus();
                self.tray_group.close_menu()
            }
            Message::WorkspaceClicked {
                is_active: true, ..
            }
            | Message::OverviewToggled => niri_action(niri::toggle_overview),
            Message::WorkspaceClicked { id, .. } => niri_action(move || niri::focus_workspace(id)),
            Message::Niri(events) => self.apply_niri_events(events),
            Message::WindowClosed(window) => self.forget_window(window),
            Message::PillBounds {
                window,
                zone,
                rects,
            } => self.blur_casters.pill_rects(
                window,
                zone,
                &rects,
                &self.config,
                &self.session.chrome,
                &self.surfaces,
            ),
            Message::Tooltip(msg) => self.update_tooltip(msg),
            Message::PointerLeft => self.dismiss_tooltip(),
            Message::TooltipHideElapsed => self.leave_tooltip_after_grace(),
            Message::Notification(msg) => self.update_notification(msg),
            Message::OutputsResolved(outputs) => self.adopt_outputs(outputs),
            Message::Redraw => self.prune_notifications(),
            Message::TooltipTick
            | Message::AnchorChange { .. }
            | Message::SetInputRegion { .. }
            | Message::AnchorSizeChange { .. }
            | Message::LayerChange { .. }
            | Message::MarginChange { .. }
            | Message::SizeChange { .. }
            | Message::ExclusiveZoneChange { .. }
            | Message::KeyboardInteractivityChange { .. }
            | Message::VirtualKeyboardPressed { .. }
            | Message::NewLayerShell { .. }
            | Message::NewBaseWindow { .. }
            | Message::NewPopUp { .. }
            | Message::NewMenu { .. }
            | Message::NewInputPanel { .. }
            | Message::RemoveWindow(_)
            | Message::ForgetLastOutput => Task::none(),
        }
    }

    fn update_idle_timeouts(&mut self, msg: IdleTimeoutsMessage) -> Task<Message> {
        self.idle_timeouts_group
            .update(msg)
            .map(Message::IdleTimeouts)
    }

    fn adopt_sound_theme(&mut self, theme: Option<String>) -> Task<Message> {
        self.notifications.adopt_sound_theme(theme);
        Task::none()
    }

    fn load_launch_tracker(&mut self, buckets: Buckets) -> Task<Message> {
        self.launch_tracker.load_from(buckets);
        self.refresh_quick_launch()
    }

    fn load_desktop_entries(&mut self, entries: Arc<DesktopEntries>) -> Task<Message> {
        self.desktop_entries.set(entries.clone());
        self.window_group.set_desktop_entries(entries);
        self.refresh_quick_launch()
    }

    fn apply_output_event(&mut self, event: LayerShellOutputEvent) -> Task<Message> {
        match event {
            LayerShellOutputEvent::Connected { name } => {
                Task::batch([self.open_bars(&name), query_outputs()])
            }
            LayerShellOutputEvent::Disconnected { name } => {
                Task::batch([self.forget_output(&name), query_outputs()])
            }
        }
    }

    fn forget_output(&mut self, output: &str) -> Task<Message> {
        let stranded: Vec<Id> = self
            .surfaces
            .iter()
            .filter(|(_, surface)| surface.output == output)
            .map(|(&id, _)| id)
            .collect();
        self.tooltip.forget_output(output);
        Task::batch(stranded.into_iter().map(|id| self.forget_window(id)))
    }

    fn forget_window(&mut self, window: Id) -> Task<Message> {
        self.surfaces.remove(&window);
        self.backdrops.remove(&window);
        self.tooltip.forget(window);
        self.tray_group.forget(window);
        self.notifications.forget(window);
        self.blur_casters.forget(window)
    }

    fn update_notification(&mut self, msg: NotificationMessage) -> Task<Message> {
        let placement = niri::Placement::derive(&self.niri);
        self.notifications.update(
            msg,
            &placement,
            self.desktop_entries.get(),
            &self.config,
            &self.session.notifications,
        )
    }

    fn prune_notifications(&mut self) -> Task<Message> {
        if !self.notifications.is_active() {
            return Task::none();
        }
        let placement = niri::Placement::derive(&self.niri);
        self.notifications
            .prune(&placement, &self.config, &self.session.notifications)
    }

    fn adopt_outputs(&mut self, outputs: HashMap<String, niri::OutputInfo>) -> Task<Message> {
        self.outputs = outputs;
        let placement = niri::Placement::derive(&self.niri);
        self.notifications
            .on_niri_changed(&placement, &self.config, &self.session.notifications)
    }

    fn update_window_group(&mut self, msg: WindowGroupMessage) -> Task<Message> {
        match self.window_group.update(msg) {
            window_group::Action::Run(task) => task.map(Message::WindowGroup),
            window_group::Action::FocusWindow(window) => {
                niri_action(move || niri::focus_window(window))
            }
            window_group::Action::ToggleWindowAudio(pid) => {
                self.audio_group.toggle_window_mute(pid).map(Message::Audio)
            }
            window_group::Action::RefreshWindowList => {
                Task::batch([self.refresh_window_assets(), self.resolve_icons()])
            }
            window_group::Action::None => Task::none(),
        }
    }

    fn update_media(&mut self, msg: MediaMessage) -> Task<Message> {
        let players_changed = matches!(msg, MediaMessage::Reading(_));
        let task = self.media_group.update(msg, &self.niri).map(Message::Media);
        if players_changed {
            Task::batch([task, self.resolve_icons()])
        } else {
            task
        }
    }

    fn update_notification_group(&mut self, msg: NotificationGroupMessage) -> Task<Message> {
        let placement = niri::Placement::derive(&self.niri);
        self.notifications.toggle(
            msg,
            &mut self.session.notifications,
            &placement,
            &self.config,
        )
    }

    fn update_tray(&mut self, msg: TrayMessage) -> Task<Message> {
        let anchor = self
            .tooltip
            .menu_anchor(self.session.chrome.margin_offset_px());
        match self.tray_group.update(msg, &self.config, anchor) {
            tray_group::Action::None => Task::none(),
            tray_group::Action::Run(task) => task,
            tray_group::Action::MenuOpened(task) => Task::batch([self.dismiss_tooltip(), task]),
            tray_group::Action::FocusWindow {
                window_app_id,
                item_id,
            } => self.focus_tray_window(&window_app_id, &item_id),
            tray_group::Action::Tooltip(message) => self.update_tooltip(message),
        }
    }

    fn focus_tray_window(&self, window_app_id: &str, item_id: &str) -> Task<Message> {
        let candidates = [String::from(window_app_id)];
        let Some(window) = niri::Placement::derive(&self.niri).window_by_source(&candidates, None)
        else {
            eprintln!("tray: {item_id}: no window with app_id {window_app_id}");
            return Task::none();
        };
        niri_action(move || niri::focus_window(window))
    }

    fn update_tooltip(&mut self, msg: TooltipMessage) -> Task<Message> {
        let departing = self.tooltip_departure();
        let TooltipMessage::Enter {
            surface,
            target,
            anchor,
        } = msg
        else {
            return self.tooltip.update(msg);
        };
        let content = self.tooltip_for(&target);
        if let Some(parent) = self.surfaces.get(&surface).filter(|_| !content.is_empty()) {
            let parent = parent.clone();
            self.tooltip.open(&parent, target, anchor, departing);
        }
        Task::none()
    }

    fn tooltip_departure(&self) -> Option<Departed> {
        let content = self
            .tooltip
            .target()
            .map(|target| self.tooltip_for(target))?;
        self.tooltip.departure(content)
    }

    fn dismiss_tooltip(&mut self) -> Task<Message> {
        let departing = self.tooltip_departure();
        self.tooltip.dismiss(departing);
        Task::none()
    }

    fn leave_tooltip_after_grace(&mut self) -> Task<Message> {
        let departing = self.tooltip_departure();
        self.tooltip.leave_elapsed(departing);
        Task::none()
    }

    fn apply_saved_panel_settings(&mut self, session: Session) -> Task<Message> {
        self.session = session;
        self.panel_settings_group.adopt(&self.session.chrome);
        Task::batch([self.remargin_surfaces(), self.rebuild_blur()])
    }

    pub(crate) fn tooltip_for(&self, target: &HoverTarget) -> Tooltip {
        if let HoverTarget::Tray(tooltip) = target {
            return (**tooltip).clone();
        }
        if let HoverTarget::Window(window) = target {
            return window_tooltip::tooltip(&self.niri, *window).unwrap_or_else(Tooltip::empty);
        }
        if let HoverTarget::Workspace(workspace) = target {
            return workspace_tooltip::tooltip(
                &self.niri_snapshot,
                &self.desktop_entries,
                *workspace,
            )
            .unwrap_or_else(Tooltip::empty);
        }
        if let HoverTarget::Output(output) = target {
            return output_group::tooltip(&self.outputs, output);
        }
        self.system_group
            .tooltip_for(target)
            .or_else(|| self.battery_group.tooltip_for(target))
            .or_else(|| self.journal_group.tooltip_for(target))
            .or_else(|| self.net_interfaces_group.tooltip_for(target))
            .or_else(|| self.audio_group.tooltip_for(target))
            .or_else(|| self.power_profile_group.tooltip_for(target))
            .or_else(|| self.monitor_group.tooltip_for(target))
            .or_else(|| self.color_temperature_group.tooltip_for(target))
            .or_else(|| self.idle_timeouts_group.tooltip_for(target))
            .or_else(|| self.internet_group.tooltip_for(target))
            .or_else(|| self.weather_group.tooltip_for(target))
            .or_else(|| self.media_group.tooltip_for(target))
            .or_else(|| self.ai_group.tooltip_for(target))
            .or_else(|| {
                self.panel_settings_group
                    .tooltip_for(target, &self.session.chrome)
            })
            .unwrap_or_else(Tooltip::empty)
    }

    pub fn any_notification_live(&self) -> bool {
        self.notifications.is_active()
    }

    pub fn any_notification_expiring(&self) -> bool {
        self.notifications.has_running_countdown()
    }

    pub fn tooltip_has_relative_time(&self) -> bool {
        self.tooltip.target().is_some_and(|target| {
            let tooltip = self.tooltip_for(target);
            tooltip.footer.is_some() || matches!(tooltip.body, TooltipBody::Log(_))
        })
    }

    fn update_color_temperature(&mut self, msg: ColorTemperatureMessage) -> Task<Message> {
        self.color_temperature_group
            .update(msg)
            .map(Message::ColorTemperature)
    }

    fn menus(&self) -> [&MenuGroup; 12] {
        [
            &self.clock_group.menu,
            &self.monitor_group.menu,
            &self.color_temperature_group.menu,
            &self.weather_group.menu,
            &self.system_group.menu,
            &self.media_group.menu,
            &self.idle_timeouts_group.menu,
            &self.panel_settings_group.menu,
            &self.internet_group.menu,
            &self.window_group.quick_launch.menu,
            &self.tray_group.menu,
            &self.ai_group.menu,
        ]
    }

    pub fn any_menu_animating(&self) -> bool {
        self.menus().iter().any(|menu| menu.is_animating())
            || self.audio_group.is_animating()
            || self.monitor_group.is_animating()
            || self.color_temperature_group.is_animating()
            || self.panel_settings_group.is_animating()
            || self.idle_timeouts_group.is_animating()
            || self.power_profile_group.is_animating()
    }

    fn dismiss_menus(&mut self) {
        self.panel_settings_group.close_menu();
        self.monitor_group.close_menu();
        self.color_temperature_group.close_menu();
        let _ = [
            &mut self.clock_group.menu,
            &mut self.weather_group.menu,
            &mut self.system_group.menu,
            &mut self.media_group.menu,
            &mut self.idle_timeouts_group.menu,
            &mut self.internet_group.menu,
            &mut self.window_group.quick_launch.menu,
            &mut self.tray_group.menu,
            &mut self.ai_group.menu,
        ]
        .map(MenuGroup::close);
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
            if let Some(delta) = self.launch_tracker.observe(&event) {
                tasks.push(Task::future(state::launches::record(delta)).discard());
            }
            let _ = self.niri.apply(event);
        }
        self.rederive_snapshot();
        if focus_change {
            self.dismiss_menus();
            tasks.push(self.tray_group.close_menu());
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
                &self.session.notifications,
            ));
        }
        Task::batch(tasks)
    }

    fn refresh_quick_launch(&mut self) -> Task<Message> {
        let Some(entries) = self.desktop_entries.arc() else {
            return Task::none();
        };
        let now = Utc::now();
        let mut outputs: Vec<String> = self
            .surfaces
            .values()
            .map(|surface| surface.output.clone())
            .collect();
        outputs.sort_unstable();
        outputs.dedup();
        let ranked = outputs
            .into_iter()
            .filter_map(|output| {
                let workspace = self.niri_snapshot.active_workspace_name(&output)?;
                let open = self.niri_snapshot.open_app_ids(&output);
                let apps = self
                    .launch_tracker
                    .rank(workspace, &open, MAX_ITEMS, now, |app_id| {
                        entries.is_launchable(app_id)
                    });
                (!apps.is_empty()).then_some((output, apps))
            })
            .collect();
        self.window_group.set_ranked(ranked);
        self.resolve_icons()
    }

    fn rederive_snapshot(&mut self) {
        self.niri_snapshot =
            niri::Snapshot::derive(&self.niri, &self.config.font, &self.font_fallbacks);
    }

    fn refresh_window_assets(&mut self) -> Task<Message> {
        let titles = niri::all_windows(&self.niri);
        Task::batch([
            self.favicons
                .poll(&self.niri)
                .map(Message::FaviconsResolved),
            self.font_fallbacks
                .resolve_missing(
                    titles.iter().flat_map(|win| win.title.chars()),
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

    fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match self
            .panel_settings_group
            .update(msg, &mut self.session.chrome)
        {
            panel_settings_group::Action::None => Task::none(),
            panel_settings_group::Action::Run(task) => task,
            panel_settings_group::Action::RebuildBlur(task) => {
                Task::batch([self.rebuild_blur(), task])
            }
            panel_settings_group::Action::Reflow(task) => Task::batch([
                self.remargin_surfaces(),
                self.blur_casters
                    .reposition(&self.session.chrome, &self.surfaces),
                task,
            ]),
        }
    }

    fn rebuild_blur(&mut self) -> Task<Message> {
        self.blur_casters
            .rebuild(&self.config, &self.session.chrome, &self.surfaces)
    }

    fn remargin_surfaces(&self) -> Task<Message> {
        let offset = self.session.chrome.margin_offset_px();
        let bars = self.surfaces.iter().map(move |(&id, surface)| {
            Task::done(Message::MarginChange {
                id,
                margin: float_margin(surface.position, offset).edges(),
            })
        });
        let tooltips = self.tooltip.surfaces().map(move |id| {
            Task::done(Message::MarginChange {
                id,
                margin: Margin::uniform(offset).edges(),
            })
        });
        Task::batch(bars.chain(tooltips))
    }

    fn open_bar(&mut self, output: &str, position: Position) -> Task<Message> {
        let height = self.config.chrome.outer_height_px as u32;
        let offset = self.session.chrome.margin_offset_px();
        let settings = layer_settings(output, position, height, float_margin(position, offset));
        let id = Id::unique();
        let surface = Surface {
            output: output.to_string(),
            position,
        };
        let strip = self.blur_casters.open_strip_for_new_bar(
            id,
            &surface,
            &self.config,
            &self.session.chrome,
        );
        self.surfaces.insert(id, surface);
        Task::batch([Task::done(Message::NewLayerShell { settings, id }), strip])
    }

    fn open_backdrop(&mut self, output: &str, position: Position) -> Task<Message> {
        let height = self.config.chrome.outer_height_px as u32;
        let settings = backdrop_shadow::settings(output, position, height);
        let id = Id::unique();
        self.backdrops.insert(id);
        Task::done(Message::NewLayerShell { settings, id })
    }

    fn open_bars(&mut self, output: &str) -> Task<Message> {
        let offset = self.session.chrome.margin_offset_px();
        let mut opens: Vec<Task<Message>> = Position::ALL
            .into_iter()
            .map(|position| self.open_bar(output, position))
            .collect();
        opens.push(self.tooltip.open_surface(output, offset));
        if self.config.chrome.shadow {
            opens.extend(
                Position::ALL
                    .into_iter()
                    .map(|position| self.open_backdrop(output, position)),
            );
        }
        Task::batch(opens)
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        if let Some(element) = self
            .tooltip
            .view(window, &self.config, &self.favicons, |target| {
                self.tooltip_for(target)
            })
        {
            return element;
        }
        if let Some(element) = self.tray_group.menu_view(window, &self.config) {
            return element;
        }
        if let Some(element) = self.notifications.view(window, &self.config) {
            return element;
        }
        view(self, window)
    }

    pub fn style(&self, _theme: &iced::Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: self.config.palette.bar_foreground,
        }
    }
}

fn query_outputs() -> Task<Message> {
    blocking("niri outputs", niri::output_info).map(Message::OutputsResolved)
}
