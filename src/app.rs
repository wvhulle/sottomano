//! The iced bar, now a multi-output daemon (Phase 1). One layer-shell surface per
//! (output, edge): `iced_layershell::build_pattern::daemon` drives many surfaces from
//! one Elm program, and `view(&self, id)` renders whichever surface iced is painting.
//!
//! Shared data (clock, battery, niri state) lives once on `Bar`; only the surface's
//! identity — which output and which edge — is keyed by `window::Id`. This mirrors
//! `../odin/shell.qml`'s `Variants { model: Quickshell.screens }` opening a top and a
//! bottom `BarWindow` per screen.

use std::collections::BTreeMap;
use std::time::Duration;

use iced::widget::{container, row, text};
use iced::window::Id;
use iced::{Alignment, Color, Element, Length, Subscription, Task};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::to_layer_message;
use niri_ipc::state::EventStreamState;

use crate::niri;
use crate::script::{self, WidgetData};

pub const BAR_HEIGHT: u32 = 32;

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

    fn label(self) -> &'static str {
        match self {
            Position::Top => "top",
            Position::Bottom => "bottom",
        }
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
    clock: String,
    battery: Option<WidgetData>,
    niri: EventStreamState,
    /// Every live layer-shell surface, keyed by its iced `window::Id`.
    surfaces: BTreeMap<Id, Surface>,
}

/// `#[to_layer_message(multi)]` injects the multi-surface action variants
/// (`NewLayerShell { settings, id }`, `AnchorSizeChange`, …) this builder needs.
#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    OpenBars(Vec<String>),                     // enumerated output names → open surfaces
    Tick,                                      // 1 Hz clock
    PollBattery,                               // 10 s cadence
    BatteryLoaded(Result<WidgetData, String>), // subprocess result
    Niri(niri_ipc::Event),                     // event-stream delta
    WindowClosed(Id),                          // surface gone (e.g. output unplugged)
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                clock: now_hm(),
                battery: None,
                niri: EventStreamState::default(),
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
                // Kick an immediate first battery poll so the widget isn't blank for 10 s.
                Task::done(Message::PollBattery),
            ]),
        )
    }

    pub fn namespace() -> String {
        "odin-rs".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenBars(outputs) => self.open_bars(outputs),
            Message::Tick => {
                self.clock = now_hm();
                Task::none()
            }
            Message::PollBattery => Task::perform(
                async { script::run("battery", &[]).await },
                Message::BatteryLoaded,
            ),
            Message::BatteryLoaded(Ok(data)) => {
                self.battery = Some(data);
                Task::none()
            }
            Message::BatteryLoaded(Err(e)) => {
                eprintln!("battery poll failed: {e}"); // keep last value, stay alive
                Task::none()
            }
            Message::Niri(event) => {
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
                let settings = layer_settings(&output, position);
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

        let battery = match &self.battery {
            Some(d) if d.available => match d.percent() {
                Some(p) => format!("{} {p}%", glyph(&d.icon)),
                None => glyph(&d.icon),
            },
            _ => "—".to_string(),
        };

        // This surface's own output — the multi-output payoff: each bar shows the
        // workspaces of the screen it lives on, not the globally-focused one.
        let ws_line = niri::workspaces_for_output(&self.niri, &surface.output)
            .iter()
            .map(|p| {
                let label = p.name.clone().unwrap_or_else(|| (p.idx + 1).to_string());
                if p.is_active {
                    format!("[{label}]")
                } else {
                    format!(" {label} ")
                }
            })
            .collect::<String>();

        // Marker names the output + edge so stacked screenshots are unambiguous next to
        // the live odin QML bars.
        let marker = format!("odin-rs {} {} ▸", surface.output, surface.position.label());
        let content = row![
            text(marker).size(16).color(Color::from_rgb(1.0, 0.6, 0.2)),
            text(battery).size(16),
            text(ws_line).size(16),
            text(&self.clock).size(16),
        ]
        .spacing(24)
        .align_y(Alignment::Center)
        .padding([0, 12]);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into()
    }

    pub fn style(&self, theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            // Opaque, distinctly-tinted backing so the odin-rs bars are unmistakable
            // when screenshotted next to the live odin QML bars.
            background_color: Color::from_rgb(0.12, 0.10, 0.20),
            text_color: theme.palette().text,
        }
    }
}

/// Layer-shell settings for one bar. `exclusive_zone: 0` keeps the spike's behaviour:
/// don't reserve space, but move out of the way of surfaces that do — so a top bar
/// lands flush below odin's reserved top zone and a bottom bar above odin's bottom one.
fn layer_settings(output: &str, position: Position) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, BAR_HEIGHT)),
        exclusive_zone: Some(0),
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
    Subscription::batch([
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
        iced::time::every(Duration::from_secs(10)).map(|_| Message::PollBattery),
        // Surface teardown (output unplugged, compositor closed it) → drop its state.
        iced::window::close_events().map(Message::WindowClosed),
        niri_subscription(),
    ])
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

/// Map an Odin icon name to a stand-in glyph. Odin renders Font Awesome via an icon
/// font; the spike just needs something visible, so a couple of ASCII/emoji stand-ins.
fn glyph(icon: &str) -> String {
    match icon {
        "plug-circle-bolt" => "⚡".to_string(),
        "car-battery" => "🔋".to_string(),
        other if other.is_empty() => "·".to_string(),
        other => other.to_string(),
    }
}
