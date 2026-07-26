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
        clock_group::{ClockGroup, ClockMessage},
        idle_timeouts_group::{IdleTimeoutsGroup, IdleTimeoutsMessage},
        internet_group::{InternetGroup, InternetMessage},
        journal_group::{JournalGroup, JournalMessage},
        media_group::{MediaGroup, MediaMessage},
        monitor_group::{MonitorGroup, MonitorMessage},
        net_interfaces_group::{NetInterfacesGroup, NetInterfacesMessage},
        panel_settings_group::{self, PanelSettingsGroup, PanelSettingsMessage},
        quick_launch_group::{self, QuickLaunchGroup, QuickLaunchMessage},
        system_group::{SystemGroup, SystemMessage},
        tray_group::{TrayGroup, TrayMessage},
        tray_menu::TrayMenuHost,
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
    tooltip::{self, HoverTarget, TooltipHost, TooltipMessage},
    widgets::{
        idle::IdleEvent,
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action as SliderAction, ControlMessage},
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
    pub(crate) monitor_group: MonitorGroup,
    pub(crate) idle_timeouts_group: IdleTimeoutsGroup,
    pub(crate) media_group: MediaGroup,
    pub(crate) audio_group: AudioGroup,
    pub(crate) tray_group: TrayGroup,
    pub(crate) tray_menu: TrayMenuHost,
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
    /// The shared `.desktop` index (app metadata + icon resolution), consumed
    /// by both the window list and the notification cards.
    pub(crate) desktop_entries: DesktopIndex,
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
    Monitor(MonitorMessage),
    IdleTimeouts(IdleTimeoutsMessage),
    Media(MediaMessage),
    Tray(TrayMessage),
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
    /// A periodic repaint while a menu animates (from the frame-timer
    /// subscription gated on [`Bar::any_menu_animating`]); its handler does
    /// nothing but trigger a redraw, and `MenuGroup::reveal` re-samples the
    /// eased curve at the current instant.
    Redraw,
    WindowClosed(Id),
    /// The pointer left a surface — closes any open tooltip that no `HoverArea`
    /// exit reached (the cursor left the bar without entering another
    /// hoverable).
    PointerLeft,
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
            monitor_group: MonitorGroup::new(config.layout.menu_expand_ms),
            idle_timeouts_group: IdleTimeoutsGroup::new(config.layout.menu_expand_ms),
            media_group: MediaGroup::new(config.layout.menu_expand_ms),
            audio_group: AudioGroup::default(),
            tray_group: TrayGroup::new(config.layout.menu_expand_ms),
            tray_menu: TrayMenuHost::default(),
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
            Message::Monitor(msg) => self.monitor_group.update(msg).map(Message::Monitor),
            Message::Media(msg) => self.media_group.update(msg).map(Message::Media),
            Message::Tray(TrayMessage::OpenMenu(index)) => self.open_tray_menu(index),
            Message::Tray(TrayMessage::MenuItemClicked(id)) => {
                if let Some((address, menu_path)) = self.tray_menu.target() {
                    self.tray_group.command(
                        crate::data_sources::tray::TrayCommand::MenuClick {
                            address,
                            menu_path,
                            id,
                        },
                    );
                }
                self.tray_menu.close()
            }
            Message::Tray(TrayMessage::ToggleSubmenu(id)) => {
                self.tray_menu.toggle_submenu(id);
                Task::none()
            }
            Message::Tray(TrayMessage::MenuResized { surface, size }) => {
                self.tray_menu.resize(surface, size, &self.config)
            }
            Message::Tray(msg) => {
                self.tray_group.update(msg);
                Task::none()
            }
            Message::IdleTimeouts(msg) => self
                .idle_timeouts_group
                .update(msg)
                .map(Message::IdleTimeouts),
            Message::Idle(IdleEvent::Connected(handle)) => self
                .idle_timeouts_group
                .connected(handle)
                .map(Message::IdleTimeouts),
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
                self.tray_menu.close()
            }
            Message::WorkspaceClicked(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(events) => self.apply_niri_events(events),
            Message::WindowClosed(window) => {
                self.surfaces.remove(&window);
                self.tooltip.forget(window);
                self.tray_menu.forget(window);
                self.notifications.forget(window);
                Task::none()
            }
            // A source with empty content suppresses its tooltip (odin drops
            // `{}`); the emptiness is decided here against the live reading.
            Message::Tooltip(TooltipMessage::Enter { target, .. })
                if self.tooltip_for(&target).is_empty() =>
            {
                Task::none()
            }
            Message::Tooltip(msg) => {
                self.tooltip
                    .update(msg, &self.surfaces, &self.outputs, &self.config)
            }
            // The pointer left a surface without crossing another hoverable (e.g.
            // straight off the bar edge), so no `HoverArea` fired its exit; close
            // any open tooltip so it can't strand.
            Message::PointerLeft => self.tooltip.close(),
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
            // Both need only a repaint: `Redraw` drives a niri-triggered menu
            // collapse frame; `TooltipTick` lets the tooltip view re-derive and
            // re-format its relative times.
            Message::Redraw | Message::TooltipTick => Task::none(),
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

    /// Live content for a hovered widget, resolved by asking each group to
    /// interpret the `target` it emitted (foreign targets yield `None`). Called
    /// only while a tooltip surface is shown, so nothing is computed when no
    /// tooltip is open. An unresolved or absent reading yields an empty
    /// tooltip.
    pub(crate) fn tooltip_for(&self, target: &HoverTarget) -> Tooltip {
        // The tray carries its own content in the target (it is static, not a
        // live reading), so it resolves here without a group lookup.
        if let HoverTarget::Tray(tooltip) = target {
            return (**tooltip).clone();
        }
        self.system_group
            .tooltip_for(target)
            .or_else(|| self.battery_group.tooltip_for(target))
            .or_else(|| self.journal_group.tooltip_for(target))
            .or_else(|| self.net_interfaces_group.tooltip_for(target))
            .or_else(|| self.audio_group.tooltip_for(target))
            .or_else(|| self.monitor_group.tooltip_for(target))
            .or_else(|| self.idle_timeouts_group.tooltip_for(target))
            .or_else(|| self.internet_group.tooltip_for(target))
            .or_else(|| self.weather_group.tooltip_for(target))
            .or_else(|| self.media_group.tooltip_for(target))
            .or_else(|| panel_settings_group::tooltip_for(target, &self.config))
            .unwrap_or_else(Tooltip::empty)
    }

    /// Whether the open tooltip (if any) has live relative time — a refresh
    /// footer or a [`TooltipBody::Log`] — to gate the 1 Hz tick that advances
    /// it.
    pub fn tooltip_has_relative_time(&self) -> bool {
        self.tooltip.target().is_some_and(|target| {
            let tooltip = self.tooltip_for(target);
            tooltip.footer.is_some() || matches!(tooltip.body, TooltipBody::Log(_))
        })
    }

    /// Open the context menu for the tray item at visible-row `index`, anchored
    /// under its icon. Re-clicking the same item closes it. The icon is hovered
    /// when right-clicked, so its tooltip anchor gives the placement (no separate
    /// bounds-tracking widget needed). A no-op for an item with no menu.
    fn open_tray_menu(&mut self, index: usize) -> Task<Message> {
        let (address, menu_path, nodes) = {
            let visible = self.tray_group.visible(&self.config);
            let Some(item) = visible.get(index) else {
                return Task::none();
            };
            match (&item.menu_path, item.menu.is_empty()) {
                (Some(path), false) => (item.address.clone(), path.clone(), item.menu.clone()),
                _ => return Task::none(),
            }
        };
        if self.tray_menu.is_open_for(&address) {
            return self.tray_menu.close();
        }
        let Some(place) = self.tooltip.menu_anchor() else {
            return Task::none();
        };
        Task::batch([
            self.tooltip.close(),
            self.tray_menu
                .open(address, menu_path, nodes, &place, &self.config),
        ])
    }

    /// The expandable groups' menus, in one place, for the cross-group reads
    /// below.
    fn menus(&self) -> [&MenuGroup; 9] {
        [
            &self.monitor_group.menu,
            &self.weather_group.menu,
            &self.system_group.menu,
            &self.media_group.menu,
            &self.idle_timeouts_group.menu,
            &self.panel_settings_group.menu,
            &self.internet_group.menu,
            &self.quick_launch.menu,
            &self.tray_group.menu,
        ]
    }

    /// Whether any group's expand curve is in flight — gates the frame-timer
    /// subscription that paints menu animations. Under layer-shell a widget
    /// can't self-drive redraws from a subscription-triggered state change,
    /// so this app-level signal drives the frames instead.
    pub fn any_menu_animating(&self) -> bool {
        self.menus().iter().any(|menu| menu.is_animating())
    }

    /// Collapse every open expandable group. Close-tasks are discarded —
    /// closing only mutates menu state (and, for idle/panel-settings, tears
    /// down a child slider); none needs a follow-up Task.
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
        let _ = self
            .media_group
            .update(MediaMessage::Menu(MenuMessage::Close));
        let _ = self
            .monitor_group
            .update(MonitorMessage::Menu(MenuMessage::Close));
        self.tray_group.update(TrayMessage::Menu(MenuMessage::Close));
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
            tasks.push(self.tray_menu.close());
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
    /// launchable apps. Cheap and synchronous; only the icon resolve it
    /// triggers runs off-thread. A no-op until the `.desktop` index has
    /// loaded.
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
        self.quick_launch
            .set_ranked(ranked)
            .map(Message::QuickLaunch)
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
                            Task::future(state::panel::save_float_offset(offset)).map(move |ok| {
                                Message::PanelSettings(PanelSettingsMessage::Offset(
                                    ControlMessage::Confirmed { value: offset, ok },
                                ))
                            }),
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

    fn open_surface(
        &mut self,
        output: &str,
        position: Position,
        kind: SurfaceKind,
    ) -> Task<Message> {
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

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        // The tooltip surface derives its content live from the hovered target,
        // so it always tracks the reading and costs nothing while unhovered.
        if let Some(target) = self.tooltip.target_for(window) {
            return tooltip::panel_surface(
                window,
                &self.tooltip_for(target),
                self.tooltip.is_settled(window),
                &self.config,
            );
        }
        if let Some(element) = self.tray_menu.view(window, &self.config) {
            return element;
        }
        if let Some(element) = self.notifications.view(window, &self.config) {
            return element;
        }
        let Some(surface) = self.surfaces.get(&window) else {
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
        // Stamp this surface onto tooltip hovers bubbling out of the bar, so
        // leaf groups need not carry it. Only the bar subtree is wrapped — the
        // tooltip panel returns early above.
        let content = tooltip::on_surface(window, content);

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
