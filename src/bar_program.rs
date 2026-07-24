//! The bar application: a multi-output daemon with one layer-shell surface per
//! output/edge.

use std::collections::BTreeMap;

use iced::{
    Alignment, Color, Element, Length, Task,
    theme::Style,
    time::Instant,
    widget::{Space, container, mouse_area, stack, text},
    window::Id,
};
use iced_layershell::{actions::LayerShellOutputEvent, to_layer_message};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};

use crate::{
    bar_layout::{
        self,
        battery_group::{BatteryGroup, BatteryMessage},
        clock_group::{ClockGroup, ClockMessage},
        panel_settings_group::{self, PanelSettingsGroup, PanelSettingsMessage},
        system_group::{SystemGroup, SystemMessage},
        window_list_group::{WindowListGroup, WindowListMessage},
    },
    config::resolved::ResolvedConfig,
    data_sources::{
        Tooltip, TooltipBody,
        app_icon::DesktopEntries,
        desktop_entries::{self, DesktopIndex},
        niri,
    },
    layer_surface::{Position, Surface, float_margin, layer_settings},
    message_subscriptions::niri_action,
    notifications::{NotificationGroup, NotificationMessage},
    tooltip::{self, TooltipHost, TooltipMessage, TooltipSource},
};

pub struct Bar {
    pub(crate) config: ResolvedConfig,
    pub(crate) battery_group: BatteryGroup,
    pub(crate) system_group: SystemGroup,
    pub(crate) panel_settings_group: PanelSettingsGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) window_list: WindowListGroup,
    pub(crate) niri: EventStreamState,
    /// Bar-facing derivation of `niri`, recomputed once per niri event so
    /// `view` reads borrowed slices instead of re-collecting per redraw.
    pub(crate) niri_snapshot: niri::Snapshot,
    /// Notification-placement facts (window edges, focus), recomputed per niri
    /// event so a toast can point back toward its producer window.
    pub(crate) niri_placement: niri::Placement,
    pub(crate) surfaces: BTreeMap<Id, Surface>,
    pub(crate) tooltip: TooltipHost,
    pub(crate) notifications: NotificationGroup,
    /// The shared `.desktop` index (app metadata + icon resolution), consumed by
    /// both the window list and the notification cards.
    pub(crate) desktop_entries: DesktopIndex,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Battery(BatteryMessage),
    System(SystemMessage),
    PanelSettings(PanelSettingsMessage),
    Clock(ClockMessage),
    WindowList(WindowListMessage),
    DismissPressed,
    WorkspaceClicked(u64),
    Niri(niri_ipc::Event),
    WindowClosed(Id),
    FrameTicked,
    /// 1 Hz tick, live only while a Log tooltip is shown, to advance its
    /// relative timestamps.
    TooltipTick,
    Tooltip(TooltipMessage),
    Notification(NotificationMessage),
    DesktopEntriesLoaded(std::sync::Arc<DesktopEntries>),
    ReloadDesktopEntries,
}

impl Bar {
    pub fn new(config: ResolvedConfig) -> (Self, Task<Message>) {
        let bar = Self {
            battery_group: BatteryGroup::default(),
            system_group: SystemGroup::new(config.layout.menu_expand_ms),
            panel_settings_group: PanelSettingsGroup::new(config.layout.menu_expand_ms),
            clock_group: ClockGroup::default(),
            window_list: WindowListGroup::default(),
            niri: EventStreamState::default(),
            niri_snapshot: niri::Snapshot::default(),
            niri_placement: niri::Placement::default(),
            surfaces: BTreeMap::new(),
            tooltip: TooltipHost::default(),
            notifications: NotificationGroup::default(),
            desktop_entries: DesktopIndex::default(),
            config,
        };
        let init = desktop_entries::load().map(Message::DesktopEntriesLoaded);
        (bar, init)
    }

    pub fn namespace() -> String {
        "odin-rs".to_string()
    }

    /// Whether any expandable group is mid-transition — drives the per-frame
    /// animation subscription. Frame ticking is an app-level concern, so it is
    /// aggregated here across all menu groups rather than exposed per group.
    pub fn is_animating(&self) -> bool {
        self.system_group.menu.is_animating() || self.panel_settings_group.menu.is_animating()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FrameTicked => {
                let now = Instant::now();
                self.system_group.menu.tick(now);
                self.panel_settings_group.menu.tick(now);
                Task::none()
            }
            Message::OutputEvent(LayerShellOutputEvent::Connected { name }) => {
                self.open_bars(&name)
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
            Message::DesktopEntriesLoaded(entries) => {
                self.desktop_entries.set(entries.clone());
                self.window_list
                    .on_entries_loaded(&self.niri, &entries)
                    .map(Message::WindowList)
            }
            Message::ReloadDesktopEntries => {
                desktop_entries::load().map(Message::DesktopEntriesLoaded)
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
            Message::DismissPressed => {
                self.panel_settings_group
                    .update(PanelSettingsMessage::Close);
                self.system_group.update(SystemMessage::Close);
                Task::none()
            }
            Message::WorkspaceClicked(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(event) => self.apply_niri_event(event),
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
            Message::Tooltip(msg) => self.tooltip.update(msg, &self.surfaces, &self.config),
            Message::Notification(msg) => self.notifications.update(
                msg,
                &self.niri,
                &self.niri_placement,
                self.desktop_entries.get(),
                &self.config,
            ),
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

    fn apply_niri_event(&mut self, event: niri_ipc::Event) -> Task<Message> {
        let window_change = matches!(
            event,
            niri_ipc::Event::WindowsChanged { .. }
                | niri_ipc::Event::WindowOpenedOrChanged { .. }
                | niri_ipc::Event::WindowClosed { .. }
        );
        let _ = self.niri.apply(event);
        self.niri_snapshot = niri::Snapshot::derive(&self.niri);
        self.niri_placement = niri::Placement::derive(&self.niri);
        let mut tasks = Vec::new();
        if window_change {
            let entries = self.desktop_entries.arc();
            tasks.push(
                self.window_list
                    .refresh(&self.niri, &self.config.font, entries.as_ref())
                    .map(Message::WindowList),
            );
        }
        // Only recompute toast placement while toasts are up — a niri change can
        // move a toast's corner or re-home it to another output.
        if self.notifications.is_active() {
            tasks.push(self.notifications.on_niri_changed(
                &self.niri,
                &self.niri_placement,
                &self.config,
            ));
        }
        Task::batch(tasks)
    }

    /// The config-mutating knobs are intercepted here rather than handled in
    /// the panel-settings group: the config and the open-surface table (needed
    /// to re-margin open surfaces) both belong to the application. Everything
    /// else is the group's own menu state.
    fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match msg {
            PanelSettingsMessage::ToggleFloating => {
                self.config.layout.floating = !self.config.layout.floating;
                self.remargin_surfaces()
            }
            PanelSettingsMessage::SetFloatOffset(offset) => {
                self.config.layout.float_offset_px = offset;
                self.remargin_surfaces()
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
            self.config.layout.float_offset_px as i32
        } else {
            0
        };
        Task::batch(self.surfaces.iter().map(|(&id, surface)| {
            Task::done(Message::MarginChange {
                id,
                margin: float_margin(surface.position, offset),
            })
        }))
    }

    fn open_bars(&mut self, output: &str) -> Task<Message> {
        let height = self.config.layout.outer_height_px as u32;
        let offset = if self.config.layout.floating {
            self.config.layout.float_offset_px as i32
        } else {
            0
        };
        let opens: Vec<Task<Message>> = Position::ALL
            .into_iter()
            .map(|position| {
                let id = Id::unique();
                let settings =
                    layer_settings(output, position, height, float_margin(position, offset));
                self.surfaces.insert(
                    id,
                    Surface {
                        output: output.to_string(),
                        position,
                    },
                );
                Task::done(Message::NewLayerShell { settings, id })
            })
            .collect();
        Task::batch(opens)
    }

    pub fn view(&self, id: Id) -> Element<'_, Message> {
        // The tooltip surface derives its content live from the hovered source,
        // so it always tracks the reading and costs nothing while unhovered.
        if let Some(source) = self.tooltip.source_for(id) {
            return tooltip::panel_surface(id, &self.tooltip_of(source), &self.config);
        }
        if let Some(element) = self.notifications.view(id, &self.config) {
            return element;
        }
        let Some(surface) = self.surfaces.get(&id) else {
            return container(text("")).into();
        };
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
