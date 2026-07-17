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

use iced::widget::{Row, Space, container, row, text};
use iced::window::Id;
use iced::{Alignment, Color, Element, Length, Subscription, Task};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::to_layer_message;
use niri_ipc::state::EventStreamState;

use crate::config::Config;
use crate::script::{self, WidgetData};
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

    pub fn label(self) -> &'static str {
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
    config: Config,
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
                config: crate::config::load(),
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

        let ctx = widget::Context {
            config: &self.config,
            clock: &self.clock,
            battery: self.battery.as_ref(),
            niri: &self.niri,
            output: &surface.output,
            position: surface.position,
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

        // The bar chrome: `surface` at `bgOpacity`, rounded only when floating — exactly
        // odin's `BarChrome`. The surface itself is fully transparent (see `style`), so
        // at `bgOpacity 0` this paints nothing and any backdrop shows through.
        container(content)
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
    /// inter-group spacing. Empty (no matching zones) collapses to a zero-width row.
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
                .map(|z| Element::from(z.view(ctx))),
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
