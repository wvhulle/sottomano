use std::{borrow::Cow, time::Duration};

use iced::{
    Element, Length, Padding, Settings, Subscription, Task, Theme,
    alignment::Vertical,
    executor,
    futures::{Stream, StreamExt, stream},
    theme::Style,
    widget::{Column, Row, container, stack},
    window,
};
use iced_test::program::{Preset, Program};
use niri_ipc::state::EventStreamState;
use tokio::time::sleep;
use vetrata_config::{Config, screen_edges::Position};
use vetrata_state::ShellSettings;
use vetrata_widgets::{
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
        text::{FontFaces, FontFallbacks},
        tooltip_body::Tooltip,
        window_icon::WindowAssets,
    },
};

use crate::{
    animation_recording::CANVAS,
    demo_script::{DEMOS, Demo},
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup},
};

const LIVE_INTERVAL: Duration = Duration::from_millis(1500);

pub struct ShowcaseProgram {
    config: Config,
    presets: Vec<Preset<ShowcaseState, Message>>,
}

impl ShowcaseProgram {
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let presets = DEMOS
            .iter()
            .map(|demo| {
                let config = config.clone();
                Preset::new(demo.name, move || ShowcaseState::boot(demo, &config))
            })
            .collect();
        ShowcaseProgram {
            config: config.clone(),
            presets,
        }
    }

    pub(crate) fn preset(&self, demo: &Demo) -> &Preset<ShowcaseState, Message> {
        self.presets
            .iter()
            .find(|preset| preset.name() == demo.name)
            .expect("every demo has a preset")
    }
}

fn live_readings(live: fn(u32) -> Vec<Message>) -> impl Stream<Item = Message> {
    stream::iter(1..)
        .then(|tick| async move {
            sleep(LIVE_INTERVAL).await;
            tick
        })
        .flat_map(move |tick| stream::iter(live(tick)))
}

pub struct ShowcaseState {
    demo: &'static Demo,
    config: Config,
    shell_settings: ShellSettings,
    desktop_entries: DesktopIndex,
    favicons: Favicons,
    font_fallbacks: FontFallbacks,
    theme: Theme,
    group: ShowcasedGroup,
    tooltip: TooltipHost,
    notifications: NotificationLayer,
}

impl ShowcaseState {
    fn boot(demo: &'static Demo, config: &Config) -> (Self, Task<Message>) {
        let mut tooltip = TooltipHost::default();
        let _ = tooltip.open_surface(SHOWCASE_OUTPUT);
        let mut shell_settings = ShellSettings::default();
        shell_settings.notifications.sound = false;
        let group = (demo.group)();
        let opening_toasts = Task::batch(group.opening_toasts().iter().map(|toast| {
            Task::done(Message::Notification(NotificationMessage::Served(
                Box::new(toast.clone()),
            )))
        }));
        let state = ShowcaseState {
            demo,
            config: config.clone(),
            shell_settings,
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            font_fallbacks: FontFallbacks::naming_theme_families(&config.theme),
            theme: Theme::from(&PaletteColors::from(&config.theme.palette)),
            group,
            tooltip,
            notifications: NotificationLayer::default(),
        };
        (state, opening_toasts)
    }

    fn toast_layer(&self) -> Element<'_, Message> {
        Element::from(self.notifications.anchored_toasts(
            SHOWCASE_OUTPUT,
            &self.config,
            &self.theme,
            &self.shell_settings,
            Padding::ZERO,
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
        );
        self.notifications.update(message, &context, &|_| None)
    }

    fn shown_tooltip(&self) -> Option<Tooltip> {
        self.tooltip
            .target()
            .and_then(|target| self.group.tooltip_for(target))
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
            .panel_groups(&self.config)
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
                fallbacks: &self.font_fallbacks,
            },
            |target| {
                self.group
                    .tooltip_for(target)
                    .unwrap_or_else(Tooltip::empty)
            },
        )
    }
}

impl Program for ShowcaseProgram {
    type State = ShowcaseState;
    type Message = Message;
    type Theme = Theme;
    type Renderer = iced::Renderer;
    type Executor = executor::Default;

    fn name() -> &'static str {
        "vetrata_widget_showcase"
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

    fn boot(&self) -> (ShowcaseState, Task<Message>) {
        ShowcaseState::boot(&DEMOS[0], &self.config)
    }

    fn presets(&self) -> &[Preset<ShowcaseState, Message>] {
        &self.presets
    }

    fn update(&self, state: &mut ShowcaseState, message: Message) -> Task<Message> {
        match message {
            Message::Tooltip(TooltipMessage::EnterAnchor {
                target,
                anchor,
                press_hints,
            }) => {
                state.group.hover(&target, &state.config);
                let panel_edge = state.panel_edge();
                state.tooltip.open(&panel_edge, target, anchor, press_hints);
                Task::none()
            }
            Message::Tooltip(message) => state.tooltip.update(message),
            Message::TooltipHideElapsed => {
                let departing = state
                    .shown_tooltip()
                    .and_then(|content| state.tooltip.departure(content));
                state.tooltip.leave_elapsed(departing);
                Task::none()
            }
            Message::Notification(message) => state.update_notification(message),
            other => {
                state.group.update(other);
                Task::none()
            }
        }
    }

    fn view<'a>(&self, state: &'a ShowcaseState, _window: window::Id) -> Element<'a, Message> {
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
        stack(
            state
                .group
                .backdrop(&state.config)
                .into_iter()
                .chain([strip, state.toast_layer()])
                .chain(state.tooltip_layer()),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn subscription(&self, state: &ShowcaseState) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run_with(state.demo.live, |&live| live_readings(live)),
            state
                .notifications
                .expiry_ticks()
                .map(Message::Notification),
        ])
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
