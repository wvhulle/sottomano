//! The bar application: a multi-output daemon with one layer-shell surface per
//! output/edge.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use chrono::Utc;
use iced::{
    Alignment, Color, Element, Length, Task,
    theme::Style,
    widget::{Space, container, mouse_area, stack, text},
    window::Id,
};
use iced_layershell::{actions::LayerShellOutputEvent, to_layer_message};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};

use crate::{
    bar_layout::{
        self,
        audio_group::{AudioGroup, AudioMessage},
        battery_group::{BatteryGroup, BatteryMessage},
        brightness_group::{BrightnessGroup, BrightnessMessage},
        clock_group::{ClockGroup, ClockMessage},
        idle_timeouts_group::{IdleTimeoutsGroup, IdleTimeoutsMessage},
        internet_group::{InternetGroup, InternetMessage},
        journal_group::{JournalGroup, JournalMessage},
        media_group::{MediaGroup, MediaMessage},
        net_interfaces_group::{NetInterfacesGroup, NetInterfacesMessage},
        panel_settings_group::{self, PanelSettingsGroup, PanelSettingsMessage},
        quick_launch_group::{self, QuickLaunchGroup, QuickLaunchMessage},
        system_group::{SystemGroup, SystemMessage},
        weather_group::{WeatherGroup, WeatherMessage},
        window_list_group::{WindowListGroup, WindowListMessage},
    },
    config::resolved::ResolvedConfig,
    data_sources::{
        Tooltip, TooltipBody,
        app_icon::DesktopEntries,
        desktop_entries::{self, DesktopIndex},
        launch_tracker::{Buckets, LaunchTracker},
        niri,
    },
    layer_surface::{
        Position, Surface, SurfaceKind, backdrop_settings, float_margin, layer_settings,
    },
    message_subscriptions::{blocking, niri_action},
    notifications::{NotificationGroup, NotificationMessage},
    state,
    tooltip::{self, TooltipHost, TooltipMessage, TooltipSource},
    widgets::{
        idle::{Arm, IdleEvent, IdleHandle},
        menu_group::MenuMessage,
        slider_control::Action as SliderAction,
    },
};

pub struct Bar {
    pub(crate) config: ResolvedConfig,
    pub(crate) weather_group: WeatherGroup,
    pub(crate) battery_group: BatteryGroup,
    pub(crate) system_group: SystemGroup,
    pub(crate) panel_settings_group: PanelSettingsGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) journal_group: JournalGroup,
    pub(crate) internet_group: InternetGroup,
    pub(crate) net_interfaces_group: NetInterfacesGroup,
    pub(crate) brightness_group: BrightnessGroup,
    pub(crate) idle_timeouts_group: IdleTimeoutsGroup,
    pub(crate) media_group: MediaGroup,
    pub(crate) audio_group: AudioGroup,
    pub(crate) window_list: WindowListGroup,
    pub(crate) quick_launch: QuickLaunchGroup,
    /// In-memory launch-frequency model feeding quick-launch, persisted to disk
    /// by [`state::launches`].
    pub(crate) launch_tracker: LaunchTracker,
    pub(crate) niri: EventStreamState,
    /// Bar-facing derivation of `niri`, recomputed once per niri event so
    /// `view` reads borrowed slices instead of re-collecting per redraw.
    pub(crate) niri_snapshot: niri::Snapshot,
    /// Per-output logical geometry, from a niri `Outputs` query — general
    /// compositor state (currently read by notification placement to centre a
    /// stack against the screen, available to any future consumer).
    pub(crate) outputs: HashMap<String, niri::OutputGeometry>,
    pub(crate) surfaces: BTreeMap<Id, Surface>,
    pub(crate) tooltip: TooltipHost,
    pub(crate) notifications: NotificationGroup,
    /// The shared `.desktop` index (app metadata + icon resolution), consumed by
    /// both the window list and the notification cards.
    pub(crate) desktop_entries: DesktopIndex,
    /// Handle to the in-process idle daemon, once it has connected to wayland.
    pub(crate) idle_handle: Option<IdleHandle>,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Weather(WeatherMessage),
    Battery(BatteryMessage),
    System(SystemMessage),
    PanelSettings(PanelSettingsMessage),
    Clock(ClockMessage),
    Journal(JournalMessage),
    Internet(InternetMessage),
    NetInterfaces(NetInterfacesMessage),
    Brightness(BrightnessMessage),
    IdleTimeouts(IdleTimeoutsMessage),
    Media(MediaMessage),
    /// The in-process idle daemon connected to wayland.
    Idle(IdleEvent),
    /// Persisted panel settings finished loading from the sqlite store.
    PanelLoaded(state::panel::PanelSettings),
    Audio(AudioMessage),
    WindowList(WindowListMessage),
    QuickLaunch(QuickLaunchMessage),
    /// The persisted launch-frequency model finished loading from disk.
    TrackerLoaded(Buckets),
    /// The shared `.desktop` index finished (re)loading.
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    DismissPressed,
    WorkspaceClicked(u64),
    Niri(Vec<niri_ipc::Event>),
    WindowClosed(Id),
    /// 1 Hz tick, live only while a Log tooltip is shown, to advance its
    /// relative timestamps.
    TooltipTick,
    Tooltip(TooltipMessage),
    Notification(NotificationMessage),
    /// The connected outputs' logical geometry arrived from a niri `Outputs`
    /// query (at boot and on output connect).
    OutputsResolved(HashMap<String, niri::OutputGeometry>),
    ReloadDesktopEntries,
}

impl Bar {
    pub fn new(config: ResolvedConfig) -> (Self, Task<Message>) {
        let bar = Self {
            weather_group: WeatherGroup::new(config.layout.menu_expand_ms),
            battery_group: BatteryGroup::default(),
            system_group: SystemGroup::new(config.layout.menu_expand_ms),
            panel_settings_group: PanelSettingsGroup::new(config.layout.menu_expand_ms),
            clock_group: ClockGroup::default(),
            journal_group: JournalGroup::default(),
            internet_group: InternetGroup::new(config.layout.menu_expand_ms),
            net_interfaces_group: NetInterfacesGroup::default(),
            brightness_group: BrightnessGroup::default(),
            idle_timeouts_group: IdleTimeoutsGroup::new(config.layout.menu_expand_ms),
            media_group: MediaGroup::new(config.layout.menu_expand_ms),
            audio_group: AudioGroup::default(),
            window_list: WindowListGroup::default(),
            quick_launch: QuickLaunchGroup::new(config.layout.menu_expand_ms),
            launch_tracker: LaunchTracker::default(),
            niri: EventStreamState::default(),
            niri_snapshot: niri::Snapshot::default(),
            outputs: HashMap::new(),
            surfaces: BTreeMap::new(),
            tooltip: TooltipHost::default(),
            notifications: NotificationGroup::default(),
            desktop_entries: DesktopIndex::default(),
            idle_handle: None,
            config,
        };
        let init = Task::batch([
            desktop_entries::load().map(Message::DesktopEntriesLoaded),
            Task::perform(state::launches::load(), Message::TrackerLoaded),
            Task::perform(state::idle::load(), |t| {
                Message::IdleTimeouts(IdleTimeoutsMessage::Loaded(t))
            }),
            Task::perform(state::panel::load(), Message::PanelLoaded),
            query_outputs(),
        ]);
        (bar, init)
    }

    pub fn namespace() -> String {
        "vetrata".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OutputEvent(LayerShellOutputEvent::Connected { name }) => {
                // Re-query output sizes too: a new output changes where a centred
                // notification stack lands.
                Task::batch([self.open_bars(&name), query_outputs()])
            }
            Message::WindowList(WindowListMessage::Clicked(window)) => {
                niri_action(move || niri::focus_window(window))
            }
            Message::WindowList(msg) => {
                let entries = self.desktop_entries.arc();
                self.window_list
                    .update(msg, &self.niri, &self.config.font, entries.as_ref())
                    .map(Message::WindowList)
            }
            Message::QuickLaunch(msg) => self.quick_launch.update(msg).map(Message::QuickLaunch),
            Message::TrackerLoaded(buckets) => {
                self.launch_tracker.load_from(buckets);
                self.refresh_quick_launch()
            }
            Message::DesktopEntriesLoaded(entries) => {
                // The app owns the reload cadence, keeps the index for the
                // launchability check, and fans it out to the icon-resolving
                // groups (window list, quick-launch). Re-rank once it lands so
                // the launchable gate applies.
                self.desktop_entries.set(entries.clone());
                Task::batch([
                    self.window_list
                        .on_entries_loaded(&self.niri, &entries)
                        .map(Message::WindowList),
                    self.quick_launch
                        .set_desktop_entries(entries)
                        .map(Message::QuickLaunch),
                    self.refresh_quick_launch(),
                ])
            }
            Message::ReloadDesktopEntries => {
                desktop_entries::load().map(Message::DesktopEntriesLoaded)
            }
            Message::Weather(msg) => {
                self.weather_group.update(msg);
                Task::none()
            }
            Message::Battery(msg) => {
                self.battery_group.update(msg);
                Task::none()
            }
            Message::System(msg) => {
                self.system_group.update(msg);
                Task::none()
            }
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::Clock(msg) => {
                self.clock_group.update(msg);
                Task::none()
            }
            Message::Journal(msg) => {
                self.journal_group.update(msg);
                Task::none()
            }
            Message::NetInterfaces(msg) => {
                self.net_interfaces_group.update(msg);
                Task::none()
            }
            Message::Internet(msg) => self.internet_group.update(msg).map(Message::Internet),
            Message::Brightness(msg) => {
                self.brightness_group.update(msg).map(Message::Brightness)
            }
            Message::Media(msg) => self.media_group.update(msg).map(Message::Media),
            Message::IdleTimeouts(msg) => {
                use crate::bar_layout::idle_timeouts_group::IdleChange;
                match self.idle_timeouts_group.update(msg) {
                    IdleChange::None => Task::none(),
                    IdleChange::Rearm => {
                        self.rearm_idle();
                        Task::none()
                    }
                    IdleChange::Persist => {
                        self.rearm_idle();
                        let timeouts = self.idle_timeouts_group.timeouts();
                        Task::future(state::idle::save(timeouts)).discard()
                    }
                }
            }
            Message::Idle(IdleEvent::Connected(handle)) => {
                self.idle_handle = Some(handle);
                self.rearm_idle();
                Task::none()
            }
            Message::PanelLoaded(settings) => {
                if let Some(floating) = settings.floating {
                    self.config.layout.floating = floating;
                }
                if let Some(offset) = settings.float_offset_px {
                    self.config.float_offset_px = offset;
                }
                self.panel_settings_group
                    .sync_offset(self.config.float_offset_px);
                self.remargin_surfaces()
            }
            Message::Audio(msg) => self.audio_group.update(msg).map(Message::Audio),
            Message::DismissPressed => {
                self.dismiss_menus();
                Task::none()
            }
            Message::WorkspaceClicked(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(events) => self.apply_niri_events(events),
            Message::WindowClosed(id) => {
                self.surfaces.remove(&id);
                self.tooltip.forget(id);
                self.notifications.forget(id);
                Task::none()
            }
            // A source with empty content suppresses its tooltip (odin drops
            // `{}`); the emptiness is decided here against the live reading.
            Message::Tooltip(TooltipMessage::Enter { source, .. })
                if self.tooltip_of(source).is_empty() =>
            {
                Task::none()
            }
            Message::Tooltip(msg) => {
                self.tooltip
                    .update(msg, &self.surfaces, &self.outputs, &self.config)
            }
            Message::Notification(msg) => {
                let placement = niri::Placement::derive(&self.niri);
                self.notifications.update(
                    msg,
                    &placement,
                    &self.outputs,
                    self.desktop_entries.get(),
                    &self.config,
                )
            }
            Message::OutputsResolved(outputs) => {
                self.outputs = outputs;
                // Re-place existing toasts now the screen size is known.
                let placement = niri::Placement::derive(&self.niri);
                self.notifications
                    .on_niri_changed(&placement, &self.outputs, &self.config)
            }
            // A redraw is all the tick needs — the tooltip view re-derives and
            // re-formats its relative times.
            Message::TooltipTick => Task::none(),
            // Disconnected outputs are handled via the WindowClosed each
            // surface receives; the rest are layer-shell actions the runtime
            // intercepts before update.
            Message::OutputEvent(LayerShellOutputEvent::Disconnected { .. })
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

    /// Live content for a hovered widget, derived from its current reading (and
    /// the owning cell's refresh footer). Called only while its tooltip surface
    /// is shown, so nothing is computed when no tooltip is open. An absent
    /// reading yields an empty tooltip.
    pub(crate) fn tooltip_of(&self, source: TooltipSource) -> Tooltip {
        match source {
            TooltipSource::Cpu => self.system_group.cpu_cell().tooltip(),
            TooltipSource::Memory => self.system_group.memory_cell().tooltip(),
            TooltipSource::Temperature => self.system_group.temperature_cell().tooltip(),
            TooltipSource::Disk => self.system_group.disk_cell().tooltip(),
            TooltipSource::Battery => self.battery_group.cell().tooltip(),
            TooltipSource::Floating => Some(panel_settings_group::floating_tooltip(&self.config)),
            TooltipSource::FloatOffset => Some(panel_settings_group::offset_tooltip(&self.config)),
            TooltipSource::JournalSystem => self.journal_group.system_cell().tooltip(),
            TooltipSource::JournalUser => self.journal_group.user_cell().tooltip(),
            TooltipSource::Bluetooth => self.net_interfaces_group.bluetooth_cell().tooltip(),
            TooltipSource::Ethernet => self.net_interfaces_group.ethernet_cell().tooltip(),
            TooltipSource::Wifi => self.net_interfaces_group.wifi_cell().tooltip(),
            TooltipSource::Mobile => self.net_interfaces_group.mobile_cell().tooltip(),
            TooltipSource::AudioSink => self.audio_group.sink_cell().tooltip(),
            TooltipSource::AudioSource => self.audio_group.source_cell().tooltip(),
            TooltipSource::Brightness => self.brightness_group.cell().tooltip(),
            TooltipSource::IdleLockUser
            | TooltipSource::IdleScreenOff
            | TooltipSource::IdleStandby
            | TooltipSource::IdlePowerDown => self.idle_timeouts_group.tooltip_for(source),
            TooltipSource::InternetDown => self.internet_group.down_cell().tooltip(),
            TooltipSource::InternetUp => self.internet_group.up_cell().tooltip(),
            TooltipSource::Tailscale => self.internet_group.tailscale_cell().tooltip(),
            TooltipSource::Wireguard(index) => {
                self.internet_group.wireguard_tooltip(index as usize)
            }
            TooltipSource::WeatherTemp => self.weather_group.temp_cell().tooltip(),
            TooltipSource::WeatherRain => self.weather_group.rain_cell().tooltip(),
            TooltipSource::PollenBirch => self.weather_group.birch_cell().tooltip(),
            TooltipSource::PollenGrass => self.weather_group.grass_cell().tooltip(),
            TooltipSource::Location => self.weather_group.location_cell().tooltip(),
            TooltipSource::Media => self.media_group.tooltip(),
        }
        .unwrap_or_else(Tooltip::empty)
    }

    /// Whether the open tooltip (if any) has live relative time — a refresh
    /// footer or a [`TooltipBody::Log`] — to gate the 1 Hz tick that advances
    /// it.
    pub fn tooltip_has_relative_time(&self) -> bool {
        self.tooltip.source().is_some_and(|source| {
            let tooltip = self.tooltip_of(source);
            tooltip.footer.is_some() || matches!(tooltip.body, TooltipBody::Log(_))
        })
    }

    /// Collapse every open expandable group. Close-tasks are discarded —
    /// closing only mutates menu state (and, for idle/panel-settings, tears down
    /// a child slider); none needs a follow-up Task.
    fn dismiss_menus(&mut self) {
        self.panel_settings_group
            .update(PanelSettingsMessage::Menu(MenuMessage::Close));
        self.system_group
            .update(SystemMessage::Menu(MenuMessage::Close));
        let _ = self
            .quick_launch
            .update(QuickLaunchMessage::Menu(MenuMessage::Close));
        let _ = self
            .idle_timeouts_group
            .update(IdleTimeoutsMessage::Menu(MenuMessage::Close));
        let _ = self
            .internet_group
            .update(InternetMessage::Menu(MenuMessage::Close));
        self.weather_group
            .update(WeatherMessage::Menu(MenuMessage::Close));
        let _ = self.media_group.update(MediaMessage::Menu(MenuMessage::Close));
    }

    /// Apply a coalesced burst of niri events as one update: fold every event
    /// into niri state, then derive and refresh **once**. A single focus/
    /// workspace action emits 3-4 events, so per-event re-derivation (each an
    /// O(all-windows) rebuild) was multiplying UI-thread work and stalling the
    /// menu-close repaint.
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
            // The active-workspace change and the open-app-set change are exactly
            // when quick-launch's ranking can change.
            workspace_change |= matches!(
                event,
                niri_ipc::Event::WorkspacesChanged { .. }
                    | niri_ipc::Event::WorkspaceActivated { .. }
            );
            focus_change |= matches!(event, niri_ipc::Event::WindowFocusChanged { .. });
            // Fold the event into the launch tracker before it is consumed; a
            // counted launch is persisted to disk (fire-and-forget).
            if let Some(delta) = self.launch_tracker.observe(&event) {
                tasks.push(Task::future(state::launches::record(delta)).discard());
            }
            let _ = self.niri.apply(event);
        }
        self.niri_snapshot = niri::Snapshot::derive(&self.niri);
        // Focus moving to another window (or to nothing) means the user left the
        // bar — dismiss any open menu group. Gated to the focus event alone:
        // window title/property churn must not collapse an open menu. The bar
        // never takes keyboard focus, so opening a menu cannot self-trigger this.
        if focus_change {
            self.dismiss_menus();
        }
        if window_change {
            let entries = self.desktop_entries.arc();
            tasks.push(
                self.window_list
                    .refresh(&self.niri, &self.config.font, entries.as_ref())
                    .map(Message::WindowList),
            );
        }
        if window_change || workspace_change {
            tasks.push(self.refresh_quick_launch());
        }
        // Toast placement is only needed while toasts are up — a niri change can
        // move a toast's corner or re-home it. Derived on demand here (and at the
        // other notification sites), never cached, so a bare focus change pays
        // nothing for it.
        if self.notifications.is_active() {
            let placement = niri::Placement::derive(&self.niri);
            tasks.push(
                self.notifications
                    .on_niri_changed(&placement, &self.outputs, &self.config),
            );
        }
        Task::batch(tasks)
    }

    /// Re-rank quick-launch for every open output from the in-memory tracker:
    /// for each distinct output (a top and bottom surface share one), rank its
    /// active workspace's candidates against the currently-open set, gated to
    /// launchable apps. Cheap and synchronous; only the icon resolve it triggers
    /// runs off-thread. A no-op until the `.desktop` index has loaded.
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
                let apps = self.launch_tracker.rank(
                    workspace,
                    &open,
                    quick_launch_group::MAX_ITEMS,
                    now,
                    |app_id| entries.is_launchable(app_id),
                );
                (!apps.is_empty()).then_some((output, apps))
            })
            .collect();
        self.quick_launch.set_ranked(ranked).map(Message::QuickLaunch)
    }

    /// The config-mutating knobs are intercepted here rather than handled in
    /// the panel-settings group: the config and the open-surface table (needed
    /// to re-margin open surfaces) both belong to the application. Everything
    /// else is the group's own menu state.
    fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match msg {
            PanelSettingsMessage::ToggleFloating => {
                self.config.layout.floating = !self.config.layout.floating;
                let floating = self.config.layout.floating;
                Task::batch([
                    self.remargin_surfaces(),
                    Task::future(state::panel::save_floating(floating)).discard(),
                ])
            }
            PanelSettingsMessage::Offset(msg) => {
                let reading = self.config.float_offset_px;
                match self.panel_settings_group.update_offset(msg, reading) {
                    SliderAction::Commit(offset) => {
                        self.config.float_offset_px = offset;
                        Task::batch([
                            self.remargin_surfaces(),
                            Task::future(state::panel::save_float_offset(offset)).discard(),
                        ])
                    }
                    _ => Task::none(),
                }
            }
            msg => {
                self.panel_settings_group.update(msg);
                Task::none()
            }
        }
    }

    /// Push the current per-subsystem timeouts (with their configured commands)
    /// to the idle daemon, if it has connected.
    fn rearm_idle(&self) {
        let Some(handle) = &self.idle_handle else {
            return;
        };
        let timeouts = self.idle_timeouts_group.timeouts();
        let bindings = &self.config.idle_bindings;
        let arms = [
            (timeouts.lock_user, &bindings.lock_user),
            (timeouts.screen_off, &bindings.screen_off),
            (timeouts.standby, &bindings.standby),
            (timeouts.power_down, &bindings.power_down),
        ]
        .into_iter()
        .filter_map(|(timeout, command)| {
            Some(Arm {
                timeout: timeout?,
                command: command.clone(),
            })
        })
        .collect();
        handle.arm(arms);
    }

    /// Re-issue each open surface's margin from the current floating/offset
    /// config — the live counterpart of the margin applied when a surface is
    /// first opened.
    fn remargin_surfaces(&self) -> Task<Message> {
        let offset = if self.config.layout.floating {
            self.config.float_offset_px as i32
        } else {
            0
        };
        Task::batch(
            self.surfaces
                .iter()
                .filter(|(_, surface)| surface.kind == SurfaceKind::Bar)
                .map(|(&id, surface)| {
                    Task::done(Message::MarginChange {
                        id,
                        margin: float_margin(surface.position, offset),
                    })
                }),
        )
    }

    fn open_surface(&mut self, output: &str, position: Position, kind: SurfaceKind) -> Task<Message> {
        let height = self.config.layout.outer_height_px as u32;
        let settings = match kind {
            SurfaceKind::Bar => {
                let offset = if self.config.layout.floating {
                    self.config.float_offset_px as i32
                } else {
                    0
                };
                layer_settings(output, position, height, float_margin(position, offset))
            }
            SurfaceKind::Backdrop => backdrop_settings(output, position, height),
        };
        let id = Id::unique();
        self.surfaces.insert(
            id,
            Surface {
                output: output.to_string(),
                position,
                kind,
            },
        );
        Task::done(Message::NewLayerShell { settings, id })
    }

    /// Open both bar strips for a new output, plus the two shadow casters when
    /// `layout.shadow` is on (the casters exist only to bloom niri's gradient
    /// drop-shadow onto the screen edge for text legibility).
    fn open_bars(&mut self, output: &str) -> Task<Message> {
        let mut opens: Vec<Task<Message>> = Position::ALL
            .into_iter()
            .map(|position| self.open_surface(output, position, SurfaceKind::Bar))
            .collect();
        if self.config.layout.shadow {
            opens.extend(
                Position::ALL
                    .into_iter()
                    .map(|position| self.open_surface(output, position, SurfaceKind::Backdrop)),
            );
        }
        Task::batch(opens)
    }

    pub fn view(&self, id: Id) -> Element<'_, Message> {
        // The tooltip surface derives its content live from the hovered source,
        // so it always tracks the reading and costs nothing while unhovered.
        if let Some(source) = self.tooltip.source_for(id) {
            return tooltip::panel_surface(
                id,
                &self.tooltip_of(source),
                self.tooltip.is_settled(id),
                &self.config,
            );
        }
        if let Some(element) = self.notifications.view(id, &self.config) {
            return element;
        }
        let Some(surface) = self.surfaces.get(&id) else {
            return container(text("")).into();
        };
        // The caster is off-screen; niri casts from its silhouette, so it only
        // needs to be fully opaque. The shadow's colour/alpha comes from the
        // layer-rule, not from this fill.
        if surface.kind == SurfaceKind::Backdrop {
            return container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Color::BLACK.into()),
                    ..Default::default()
                })
                .into();
        }
        let content = match surface.position {
            Position::Top => bar_layout::top_row(self, surface.position),
            Position::Bottom => bar_layout::bottom_row(self, &surface.output, surface.position),
        };
        // Stamp this surface's id onto tooltip hovers bubbling out of the bar,
        // so leaf groups need not carry it. Only the bar subtree is wrapped —
        // the tooltip panel returns early above.
        let content = tooltip::on_surface(id, content);

        let dismiss = mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::DismissPressed);

        container(stack![dismiss, content])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .style(|_| container::Style {
                background: Some(self.config.palette.bar_background.into()),
                border: iced::Border {
                    radius: self.config.chrome_radius().into(),
                    ..Default::default()
                },
                text_color: Some(self.config.palette.bar_foreground),
                ..Default::default()
            })
            .into()
    }

    pub fn style(&self, _theme: &iced::Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: self.config.palette.bar_foreground,
        }
    }
}

/// Query niri for the connected outputs' logical geometry off-thread, into the
/// app's shared output registry.
fn query_outputs() -> Task<Message> {
    blocking("niri outputs", niri::output_geometry).map(Message::OutputsResolved)
}
