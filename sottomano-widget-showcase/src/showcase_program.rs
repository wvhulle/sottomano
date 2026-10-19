use std::borrow::Cow;

use iced::{
    Element, Length, Padding, Settings, Subscription, Task, Theme,
    alignment::Vertical,
    executor,
    theme::Style,
    time::every,
    widget::{Column, Row, container, stack},
    window,
};
use iced_test::program::{Preset, Program};
use niri_ipc::state::EventStreamState;
use sottomano_config::{Config, screen_edges::Position};
use sottomano_state::ShellSettings;
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    hover_tooltips::{TooltipHost, TooltipMessage},
    layer_surfaces::PanelEdge,
    notification_toasts::{NotificationLayer, ToastContext},
    shell_message::Message,
    toast_messages::NotificationMessage,
    widgets::{
        favicon::Favicons,
        palette_colors::PaletteColors,
        panel_frame::{LayerFrameStyle, OutlinedSides, panel_frame},
        text::FontFaces,
        tooltip_body::Tooltip,
        window_icon::WindowAssets,
    },
};

use crate::{
    animation_recording::{CANVAS, FRAME_INTERVAL},
    demo_script::{DEMOS, Demo},
    fixtures::{sample, showcase_epoch},
    scripted_frames::scripted_frames,
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup},
};

#[must_use]
pub fn booting_clock() -> Clock {
    Clock {
        wall: showcase_epoch(),
        ..Clock::real()
    }
}

#[derive(Debug, Clone)]
pub enum ShowcaseMessage {
    Frame(Clock),
    Shell(Box<Message>),
}

impl From<Message> for ShowcaseMessage {
    fn from(message: Message) -> Self {
        ShowcaseMessage::Shell(Box::new(message))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pacing {
    Realtime,
    Scripted,
}

pub struct ShowcaseProgram {
    config: Config,
    presets: Vec<Preset<ShowcaseState, ShowcaseMessage>>,
    pacing: Pacing,
    start: Clock,
}

impl ShowcaseProgram {
    #[must_use]
    pub fn interactive(config: &Config) -> Self {
        Self::with_pacing(config, Pacing::Realtime, booting_clock())
    }

    #[must_use]
    pub fn scripted(config: &Config, start: Clock) -> Self {
        Self::with_pacing(config, Pacing::Scripted, start)
    }

    fn with_pacing(config: &Config, pacing: Pacing, start: Clock) -> Self {
        let presets = DEMOS
            .iter()
            .map(|demo| {
                let config = config.clone();
                Preset::new(demo.name, move || ShowcaseState::boot(demo, &config, start))
            })
            .collect();
        ShowcaseProgram {
            config: config.clone(),
            presets,
            pacing,
            start,
        }
    }

    pub(crate) fn preset(&self, demo: &Demo) -> &Preset<ShowcaseState, ShowcaseMessage> {
        self.presets
            .iter()
            .find(|preset| preset.name() == demo.name)
            .expect("every demo has a preset")
    }
}

pub struct ShowcaseState {
    demo: &'static Demo,
    config: Config,
    shell_settings: ShellSettings,
    desktop_entries: DesktopIndex,
    favicons: Favicons,
    theme: Theme,
    group: ShowcasedGroup,
    tooltip: TooltipHost,
    notifications: NotificationLayer,
    clock: Clock,
    booted_at: Clock,
    delivered_sample: u32,
}

impl ShowcaseState {
    fn boot(demo: &'static Demo, config: &Config, clock: Clock) -> (Self, Task<ShowcaseMessage>) {
        FontFaces::point_generic_families_at(&config.theme);
        let mut tooltip = TooltipHost::default();
        let _ = tooltip.open_surface(SHOWCASE_OUTPUT);
        let mut shell_settings = ShellSettings::default();
        shell_settings.notifications.sound = false;
        let group = (demo.group)(clock);
        let opening_toasts = Task::batch(group.opening_toasts().iter().map(|toast| {
            Task::done(ShowcaseMessage::from(Message::Notification(
                NotificationMessage::Served(Box::new(toast.clone())),
            )))
        }));
        let state = ShowcaseState {
            demo,
            config: config.clone(),
            shell_settings,
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            theme: Theme::from(&PaletteColors::from(&config.theme.palette)),
            group,
            tooltip,
            notifications: NotificationLayer::default(),
            clock,
            booted_at: clock,
            delivered_sample: sample(clock),
        };
        (state, opening_toasts)
    }

    fn update_shell(&mut self, message: Message) -> Task<ShowcaseMessage> {
        match message {
            Message::Tooltip(TooltipMessage::EnterAnchor {
                target,
                anchor,
                press_hints,
            }) => {
                self.group.hover(&target, &self.config, self.clock);
                let panel_edge = self.panel_edge();
                self.tooltip.open(&panel_edge, target, anchor, press_hints);
                Task::none()
            }
            Message::Tooltip(message) => {
                let _hide_due = self.tooltip.update(message, self.clock.instant);
                Task::none()
            }
            Message::TooltipHideElapsed => {
                let departing = self
                    .shown_tooltip()
                    .and_then(|content| self.tooltip.departure(content));
                self.tooltip.leave_elapsed(self.clock.instant, departing);
                Task::none()
            }
            Message::Notification(message) => {
                self.update_notification(message).map(ShowcaseMessage::from)
            }
            other => {
                self.group.update(other, self.clock);
                Task::none()
            }
        }
    }

    fn advance_frame(&mut self, frame: Clock) -> Task<ShowcaseMessage> {
        let elapsed = frame
            .instant
            .saturating_duration_since(self.booted_at.instant);
        let clock = self.booted_at.advanced(elapsed);
        self.clock = clock;
        let mut tasks = Vec::new();
        if sample(clock) > self.delivered_sample {
            self.delivered_sample = sample(clock);
            for reading in (self.demo.live)(clock) {
                tasks.push(self.update_shell(reading));
            }
        }
        if self
            .tooltip
            .hide_due()
            .is_some_and(|due| due <= clock.instant)
        {
            tasks.push(self.update_shell(Message::TooltipHideElapsed));
        }
        if self
            .notifications
            .next_expiry()
            .is_some_and(|due| due <= clock.instant)
        {
            tasks.push(self.update_shell(Message::Notification(NotificationMessage::ExpiryDue)));
        }
        Task::batch(tasks)
    }

    fn toast_layer(&self) -> Element<'_, Message> {
        Element::from(self.notifications.anchored_toasts(
            SHOWCASE_OUTPUT,
            &self.config,
            &self.theme,
            &self.shell_settings,
            Padding::ZERO,
            self.clock,
        ))
        .map(Message::Notification)
    }

    fn update_notification(&mut self, message: NotificationMessage) -> Task<Message> {
        let unplaced = EventStreamState::default();
        let context = ToastContext::new(
            self.group.niri().unwrap_or(&unplaced),
            self.desktop_entries.get(),
            &self.config,
            &self.shell_settings,
            false,
            self.clock,
        );
        self.notifications.update(message, &context, &|_| None)
    }

    fn shown_tooltip(&self) -> Option<Tooltip> {
        self.tooltip
            .target()
            .and_then(|target| self.group.tooltip_for(target, self.clock))
    }

    fn panel_edge(&self) -> PanelEdge {
        PanelEdge {
            output: SHOWCASE_OUTPUT.to_owned(),
            position: self.group.position(),
        }
    }

    fn group_strip(&self) -> Element<'_, Message> {
        let panel = &self.config.layout.panel;
        let position = self.group.position();
        let layer = match position {
            Position::Left => &self.shell_settings.sidebar,
            Position::Top | Position::Bottom => &self.shell_settings.panels,
        };
        let frame_style = LayerFrameStyle::new(layer, &self.config).container_style();
        let outlined_sides =
            OutlinedSides::inward_of(position).all_when_floating(&self.shell_settings.layout);
        let framed = self
            .group
            .panel_groups(&self.config, self.clock)
            .into_iter()
            .map(|group| {
                let content = match position {
                    Position::Left => container(group).padding([layer.corner_radius_px, 0.0]),
                    Position::Top | Position::Bottom => container(group)
                        .height(Length::Fixed(panel.height))
                        .align_y(Vertical::Center)
                        .padding([0.0, layer.frame_corner_inset() + panel.group_padding]),
                };
                panel_frame(content, &frame_style)
                    .outlined_sides(outlined_sides)
                    .into()
            });
        match position {
            Position::Left => Column::with_children(framed)
                .spacing(panel.widget_spacing)
                .into(),
            Position::Top | Position::Bottom => Row::with_children(framed)
                .spacing(panel.widget_spacing)
                .into(),
        }
    }

    fn tooltip_layer(&self) -> Option<Element<'_, Message>> {
        let surface = self.tooltip.surface_on(SHOWCASE_OUTPUT)?;
        self.tooltip.view(
            surface,
            &self.config,
            &self.shell_settings,
            WindowAssets {
                entries: &self.desktop_entries,
                favicons: &self.favicons,
            },
            |target| {
                self.group
                    .tooltip_for(target, self.clock)
                    .unwrap_or_else(Tooltip::empty)
            },
            self.clock,
        )
    }
}

impl Program for ShowcaseProgram {
    type State = ShowcaseState;
    type Message = ShowcaseMessage;
    type Theme = Theme;
    type Renderer = iced::Renderer;
    type Executor = executor::Default;

    fn name() -> &'static str {
        "sottomano_widget_showcase"
    }

    fn settings(&self) -> Settings {
        Settings {
            default_font: FontFaces::from(&self.config.theme.font).regular,
            fonts: vec![Cow::Owned(self.config.theme.font.data.to_vec())],
            antialiasing: true,
            ..Settings::default()
        }
    }

    fn window(&self) -> Option<window::Settings> {
        Some(window::Settings {
            size: CANVAS,
            ..window::Settings::default()
        })
    }

    fn boot(&self) -> (ShowcaseState, Task<ShowcaseMessage>) {
        ShowcaseState::boot(&DEMOS[0], &self.config, self.start)
    }

    fn presets(&self) -> &[Preset<ShowcaseState, ShowcaseMessage>] {
        &self.presets
    }

    fn update(&self, state: &mut ShowcaseState, message: ShowcaseMessage) -> Task<ShowcaseMessage> {
        match message {
            ShowcaseMessage::Frame(clock) => state.advance_frame(clock),
            ShowcaseMessage::Shell(message) => state.update_shell(*message),
        }
    }

    fn view<'a>(
        &self,
        state: &'a ShowcaseState,
        _window: window::Id,
    ) -> Element<'a, ShowcaseMessage> {
        let gap = state.config.layout.panel.group_padding;
        let panel_side = match state.group.position() {
            Position::Top => Vertical::Top,
            Position::Left => Vertical::Center,
            Position::Bottom => Vertical::Bottom,
        };
        let strip = container(state.group_strip())
            .padding(gap)
            .height(Length::Fill)
            .align_y(panel_side)
            .into();
        let layers = stack(
            state
                .group
                .backdrop(&state.config)
                .into_iter()
                .chain([strip, state.toast_layer()])
                .chain(state.tooltip_layer()),
        )
        .width(Length::Fill)
        .height(Length::Fill);
        scripted_frames(state.clock.instant, layers).map(ShowcaseMessage::from)
    }

    fn subscription(&self, _state: &ShowcaseState) -> Subscription<ShowcaseMessage> {
        match self.pacing {
            Pacing::Realtime => {
                every(FRAME_INTERVAL).map(|_| ShowcaseMessage::Frame(Clock::real()))
            }
            Pacing::Scripted => Subscription::none(),
        }
    }

    fn theme(&self, state: &ShowcaseState, _window: window::Id) -> Option<Theme> {
        Some(state.theme.clone())
    }

    fn style(&self, state: &ShowcaseState, _theme: &Theme) -> Style {
        let palette = PaletteColors::from(&state.config.theme.palette);
        Style {
            background_color: palette.surface_raised,
            text_color: palette.bar_foreground,
        }
    }
}
