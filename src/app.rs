//! The iced bar: a multi-output daemon (Phase 1) fed by native data sources (Phase 2).
//! One layer-shell surface per (output, edge): `iced_layershell::build_pattern::daemon`
//! drives many surfaces from one Elm program, and `view(&self, id)` renders whichever
//! surface iced is painting.
//!
//! Shared data (clock, the polled `source` widgets, niri state) lives once on `Bar`;
//! only the surface's identity — which output and which edge — is keyed by `window::Id`.
//! This mirrors `../odin/shell.qml`'s `Variants { model: Quickshell.screens }` opening a
//! top and a bottom `BarWindow` per screen.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use iced::widget::{Row, Space, container, mouse_area, row, stack, text};
use iced::window::Id;
use iced::{Alignment, Color, Element, Length, Subscription, Task};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::to_layer_message;
use niri_ipc::state::EventStreamState;

use crate::config::Config;
use crate::desktop::DesktopEntries;
use crate::source::{Source, disk, sysfs};
use crate::{niri, widget};

/// Bar foreground: fixed white, mirroring odin's `Config.barText`. The bar sits over a
/// (blurred/shadowed) backdrop rather than a `surface`-colored panel, so it wants a
/// constant light foreground regardless of which palette is active.
const BAR_TEXT: Color = Color::WHITE;

/// Which screen edge a surface anchors to. Odin opens both per screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    const ALL: [Position; 2] = [Position::Top, Position::Bottom];

    /// Full-width bar anchored to this edge; left+right makes width compositor-driven.
    fn anchor(self) -> Anchor {
        let edge = match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
        };
        edge | Anchor::Left | Anchor::Right
    }
}

/// The identity of one open surface: which output it lives on and which edge it hugs.
/// `view` renders that output's workspaces — the whole point of going multi-output.
#[derive(Debug, Clone)]
struct Surface {
    output: String,
    position: Position,
}

pub struct Bar {
    config: Config,
    clock: String,
    /// Polled native sources, `None` until first poll (or when unavailable). Only the
    /// sources odin's bar composition uses (its `System` group + `Battery`).
    battery: Option<sysfs::Battery>,
    cpu: Option<sysfs::CpuUsage>,
    memory: Option<sysfs::Memory>,
    temperature: Option<sysfs::Temperature>,
    disk: Option<disk::Disk>,
    /// The one expanded menu group, if any (odin's single-open `GroupMenuState`).
    open_menu: Option<widget::MenuId>,
    /// The menu group under the pointer, for the hairline's hover state.
    hovered_menu: Option<widget::MenuId>,
    niri: EventStreamState,
    /// Installed `.desktop` entries for window→icon resolution, scanned once at startup
    /// (odin's `DesktopEntries` singleton); `None` until that scan completes.
    desktop_entries: Option<Arc<DesktopEntries>>,
    /// Every live layer-shell surface, keyed by its iced `window::Id`.
    surfaces: BTreeMap<Id, Surface>,
}

/// The native sources the bar polls. One timer per kind fires `Message::Poll(kind)`.
#[derive(Debug, Clone, Copy)]
pub enum SourceKind {
    Battery,
    Cpu,
    Memory,
    Temperature,
    Disk,
}

impl SourceKind {
    const ALL: [SourceKind; 5] = [
        SourceKind::Battery,
        SourceKind::Cpu,
        SourceKind::Memory,
        SourceKind::Temperature,
        SourceKind::Disk,
    ];

    /// Poll cadence per source — faster for volatile readings (cpu load), slower for
    /// ones that drift (battery, disk).
    fn interval(self) -> Duration {
        let secs = match self {
            SourceKind::Cpu => 3,
            SourceKind::Memory | SourceKind::Temperature => 5,
            SourceKind::Battery => 30,
            SourceKind::Disk => 60,
        };
        Duration::from_secs(secs)
    }
}

/// `#[to_layer_message(multi)]` injects the multi-surface action variants
/// (`NewLayerShell { settings, id }`, `AnchorSizeChange`, …) this builder needs.
#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    OpenBars(Vec<String>),                        // enumerated output names → open surfaces
    DesktopEntriesLoaded(Arc<DesktopEntries>),     // .desktop scan finished
    Tick,                                          // 1 Hz clock
    Poll(SourceKind),      // a source's timer fired
    // Poll results — typed per source (trait objects can't satisfy Message: Clone).
    BatteryLoaded(Option<sysfs::Battery>),
    CpuLoaded(Option<sysfs::CpuUsage>),
    MemoryLoaded(Option<sysfs::Memory>),
    TemperatureLoaded(Option<sysfs::Temperature>),
    DiskLoaded(Option<disk::Disk>),
    ToggleMenu(widget::MenuId),         // click a menu handle → open/close it
    HoverMenu(Option<widget::MenuId>),  // pointer entered/left a menu (hairline state)
    CloseMenus,                         // click bar background → collapse any open menu
    FocusWindow(u64),                   // click a window-list entry → focus that window
    FocusWorkspace(u64),                // click a workspace pill → focus that workspace
    Niri(niri_ipc::Event), // event-stream delta
    WindowClosed(Id),      // surface gone (e.g. output unplugged)
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                config: crate::config::load(),
                clock: now_hm(),
                battery: None,
                cpu: None,
                memory: None,
                temperature: None,
                disk: None,
                open_menu: None,
                hovered_menu: None,
                niri: EventStreamState::default(),
                desktop_entries: None,
                surfaces: BTreeMap::new(),
            },
            Task::batch([
                // Enumerate outputs off the async executor (blocking niri socket), then
                // open the bars. Log-and-empty on failure: no outputs → no bars, rather
                // than a crash.
                Task::perform(
                    async { tokio::task::spawn_blocking(niri::list_outputs).await },
                    |joined| match joined {
                        Ok(Ok(names)) => Message::OpenBars(names),
                        Ok(Err(e)) => {
                            eprintln!("niri: {e}");
                            Message::OpenBars(Vec::new())
                        }
                        Err(e) => {
                            eprintln!("niri: output enumeration task panicked: {e}");
                            Message::OpenBars(Vec::new())
                        }
                    },
                ),
                // Scan installed .desktop entries once (blocking directory walk) for the
                // window list's icon resolution.
                Task::perform(
                    async { tokio::task::spawn_blocking(DesktopEntries::load).await },
                    |joined| match joined {
                        Ok(entries) => Message::DesktopEntriesLoaded(Arc::new(entries)),
                        Err(e) => {
                            eprintln!("desktop entries: scan task panicked: {e}");
                            Message::DesktopEntriesLoaded(Arc::new(DesktopEntries::empty()))
                        }
                    },
                ),
                // Kick an immediate first poll of every source so widgets aren't blank.
                Task::batch(SourceKind::ALL.map(|kind| Task::done(Message::Poll(kind)))),
            ]),
        )
    }

    pub fn namespace() -> String {
        "odin-rs".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenBars(outputs) => self.open_bars(outputs),
            Message::DesktopEntriesLoaded(entries) => {
                self.desktop_entries = Some(entries);
                Task::none()
            }
            Message::Tick => {
                self.clock = now_hm();
                Task::none()
            }
            Message::Poll(kind) => match kind {
                SourceKind::Battery => poll::<sysfs::Battery>().map(Message::BatteryLoaded),
                SourceKind::Cpu => poll::<sysfs::CpuUsage>().map(Message::CpuLoaded),
                SourceKind::Memory => poll::<sysfs::Memory>().map(Message::MemoryLoaded),
                SourceKind::Temperature => {
                    poll::<sysfs::Temperature>().map(Message::TemperatureLoaded)
                }
                SourceKind::Disk => poll::<disk::Disk>().map(Message::DiskLoaded),
            },
            Message::BatteryLoaded(data) => {
                self.battery = data;
                Task::none()
            }
            Message::CpuLoaded(data) => {
                self.cpu = data;
                Task::none()
            }
            Message::MemoryLoaded(data) => {
                self.memory = data;
                Task::none()
            }
            Message::TemperatureLoaded(data) => {
                self.temperature = data;
                Task::none()
            }
            Message::DiskLoaded(data) => {
                self.disk = data;
                Task::none()
            }
            Message::ToggleMenu(id) => {
                // Single-open: toggling the open one closes it, else this becomes the open one.
                self.open_menu = (self.open_menu != Some(id)).then_some(id);
                Task::none()
            }
            Message::HoverMenu(id) => {
                self.hovered_menu = id;
                Task::none()
            }
            Message::CloseMenus => {
                self.open_menu = None;
                Task::none()
            }
            Message::FocusWindow(window) => niri_action(move || niri::focus_window(window)),
            Message::FocusWorkspace(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(event) => {
                if matches!(event, niri_ipc::Event::WindowFocusChanged { .. }) {
                    eprintln!("app: update got WindowFocusChanged at {:?}", std::time::SystemTime::now());
                }
                niri::apply(&mut self.niri, event);
                Task::none()
            }
            Message::WindowClosed(id) => {
                self.surfaces.remove(&id);
                Task::none()
            }
            // Layer-shell action variants injected by the macro: no-op here.
            _ => Task::none(),
        }
    }

    /// Open one bar per (output, edge). Each gets a fresh `window::Id` registered in
    /// `surfaces` *before* the surface exists, so the `view(id)` that follows the
    /// compositor's configure already knows what to draw.
    fn open_bars(&mut self, outputs: Vec<String>) -> Task<Message> {
        let opens: Vec<Task<Message>> = outputs
            .iter()
            .flat_map(|output| {
                Position::ALL
                    .into_iter()
                    .map(move |position| (output.clone(), position))
            })
            .map(|(output, position)| {
                let id = Id::unique();
                let settings = layer_settings(&output, position, self.config.bar_outer_height());
                self.surfaces.insert(id, Surface { output, position });
                Task::done(Message::NewLayerShell { settings, id })
            })
            .collect();
        Task::batch(opens)
    }

    pub fn view(&self, id: Id) -> Element<'_, Message> {
        let Some(surface) = self.surfaces.get(&id) else {
            // A surface iced knows about but we don't (e.g. an AllScreens hotplug we
            // never registered). Render nothing rather than panic.
            return container(text("")).into();
        };
        if matches!(surface.position, Position::Bottom) {
            eprintln!("app: view(bottom) called at {:?}", std::time::SystemTime::now());
        }

        let ctx = widget::Context {
            config: &self.config,
            clock: &self.clock,
            battery: self.battery.as_ref().map(widget::as_bar_widget),
            cpu: self.cpu.as_ref().map(widget::as_bar_widget),
            memory: self.memory.as_ref().map(widget::as_bar_widget),
            temperature: self.temperature.as_ref().map(widget::as_bar_widget),
            disk: self.disk.as_ref().map(widget::as_bar_widget),
            niri: &self.niri,
            desktop_entries: self.desktop_entries.as_deref(),
            output: &surface.output,
            position: surface.position,
            open_menu: self.open_menu,
            hovered_menu: self.hovered_menu,
        };
        let zones = widget::layout(surface.position);

        // Compose the zones into a strip: start-anchored group, fill, center group,
        // fill, end-anchored group — the iced stand-in for odin's per-zone edge anchors.
        let content = row![
            self.zone_group(&zones, &ctx, widget::Align::Start),
            Space::new().width(Length::Fill),
            self.zone_group(&zones, &ctx, widget::Align::Center),
            Space::new().width(Length::Fill),
            self.zone_group(&zones, &ctx, widget::Align::End),
        ]
        .align_y(Alignment::Center)
        .padding([
            self.config.layout.vertical_padding as f32,
            self.config.layout.horizontal_group_padding as f32,
        ]);

        // Click-anywhere-empty collapses any open menu (odin's dismiss MouseArea). It sits
        // *under* the content, so widget/menu-handle clicks are captured by their own
        // mouse areas and only gap clicks fall through to here.
        let dismiss = mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::CloseMenus);

        // The bar chrome: `surface` at `bgOpacity`, rounded only when floating — exactly
        // odin's `BarChrome`. The surface itself is fully transparent (see `style`), so
        // at `bgOpacity 0` this paints nothing and any backdrop shows through.
        container(stack![dismiss, content])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .style(|_| container::Style {
                background: Some(self.config.chrome_color().into()),
                border: iced::Border {
                    radius: self.config.chrome_radius().into(),
                    ..Default::default()
                },
                text_color: Some(BAR_TEXT),
                ..Default::default()
            })
            .into()
    }

    /// Render all zones with a given alignment into one row, separated by odin's
    /// inter-group spacing. Hidden zones (all groups unavailable) are skipped.
    fn zone_group<'a>(
        &self,
        zones: &[widget::Zone],
        ctx: &widget::Context<'a>,
        align: widget::Align,
    ) -> Row<'a, Message> {
        Row::with_children(
            zones
                .iter()
                .filter(|z| z.align == align)
                .filter_map(|z| z.view(ctx))
                .map(Element::from),
        )
        .spacing(self.config.layout.horizontal_group_spacing as f32)
        .align_y(Alignment::Center)
    }

    pub fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            // Fully transparent layer surface — the chrome fill (surface at bgOpacity)
            // is painted by the view's container so `bgOpacity 0` yields a genuinely
            // transparent bar, like odin. Requires the compositor to composite alpha.
            background_color: Color::TRANSPARENT,
            text_color: BAR_TEXT,
        }
    }
}

/// Layer-shell settings for one bar. The bar reserves its own strip via an exclusive
/// zone equal to its height (odin's `exclusiveZone: barOuterHeight`), so niri tiles
/// windows clear of it instead of letting them overlap. `height` is odin's
/// `barOuterHeight` from the shared config; it also stacks below odin's own reserved
/// zone, so the two bars sit flush without overlapping windows.
fn layer_settings(output: &str, position: Position, height: u32) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, height)),
        exclusive_zone: Some(height as i32),
        anchor: position.anchor(),
        layer: Layer::Top,
        // Never take keyboard focus — mirrors Odin's WlrKeyboardFocus.None.
        keyboard_interactivity: KeyboardInteractivity::None,
        // Bind this surface to a specific niri output by name.
        output_option: OutputOption::OutputName(output.to_string()),
        ..Default::default()
    }
}

pub fn subscription(_state: &Bar) -> Subscription<Message> {
    // One poll timer per source at its own cadence (`SourceKind::interval`), plus the
    // clock, surface teardown, and the niri event stream. iced requires each `map`
    // closure to be a zero-sized non-capturing literal, so the timers are spelled out
    // rather than built in a loop (a captured `kind` or an `fn` pointer both fail).
    use SourceKind::*;
    Subscription::batch([
        iced::time::every(Battery.interval()).map(|_| Message::Poll(Battery)),
        iced::time::every(Cpu.interval()).map(|_| Message::Poll(Cpu)),
        iced::time::every(Memory.interval()).map(|_| Message::Poll(Memory)),
        iced::time::every(Temperature.interval()).map(|_| Message::Poll(Temperature)),
        iced::time::every(Disk.interval()).map(|_| Message::Poll(Disk)),
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
        iced::window::close_events().map(Message::WindowClosed),
        niri_subscription(),
    ])
}

/// Poll a native source off the async executor and deliver its `Option` result. A read
/// runs on a blocking thread (some read `/proc` twice with a delay); a join failure
/// degrades to `None` (source treated as unavailable this cycle) rather than a crash.
/// Fire-and-forget a blocking niri action off the async executor; the resulting state
/// change arrives back through the event stream, so no result message is produced.
fn niri_action(action: impl FnOnce() + Send + 'static) -> Task<Message> {
    Task::future(async move {
        let _ = tokio::task::spawn_blocking(action).await;
    })
    .discard()
}

fn poll<S: Source + Send + 'static>() -> Task<Option<S>> {
    Task::perform(
        async { tokio::task::spawn_blocking(S::poll).await },
        |joined| match joined {
            Ok(data) => data,
            Err(e) => {
                eprintln!("source {}: poll task panicked: {e}", S::NAME);
                None
            }
        },
    )
}

/// Bridge the blocking niri event thread into an iced Subscription. The dedicated
/// thread `blocking_send`s events into a tokio channel; here we `recv().await` and
/// forward each as `Message::Niri`.
fn niri_subscription() -> Subscription<Message> {
    use iced::futures::SinkExt;
    Subscription::run(|| {
        iced::stream::channel(256, |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            let mut rx = niri::spawn_event_thread();
            while let Some(event) = rx.recv().await {
                if output.send(Message::Niri(event)).await.is_err() {
                    break;
                }
            }
        })
    })
}

fn now_hm() -> String {
    // Minimal, dependency-free HH:MM from the system clock (UTC). A real bar would use
    // chrono with the local timezone; the spike only needs a ticking value.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    format!("{h:02}:{m:02}")
}
