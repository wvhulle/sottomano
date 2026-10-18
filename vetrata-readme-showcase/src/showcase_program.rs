use std::iter::once;

use iced::{
    Element, Length, Settings, Task, Theme,
    alignment::Vertical,
    executor,
    theme::Style,
    widget::{Column, Row, container, stack},
    window,
};
use iced_test::program::Program;
use vetrata_config::{Config, screen_edges::Position};
use vetrata_sources::desktop_entries::DesktopIndex;
use vetrata_state::{OutlinedSides, ShellSettings};
use vetrata_widgets::{
    hover_tooltips::{TooltipHost, TooltipMessage},
    layer_surfaces::PanelEdge,
    shell_message::Message,
    widgets::{
        favicon::Favicons, panel_frame::panel_frame, text::FontFallbacks, tooltip_body::Tooltip,
        window_icon::WindowAssets,
    },
};

use crate::{
    demo_script::Demo,
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup},
};

pub struct ShowcaseProgram {
    pub demo: &'static Demo,
    pub config: Config,
}

pub struct ShowcaseState {
    config: Config,
    shell_settings: ShellSettings,
    desktop_entries: DesktopIndex,
    favicons: Favicons,
    font_fallbacks: FontFallbacks,
    theme: Theme,
    group: ShowcasedGroup,
    tooltip: TooltipHost,
}

impl ShowcaseState {
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
        let frame_style = layer.frame_style(&self.config);
        let outlined_sides = self
            .shell_settings
            .outlined_sides(OutlinedSides::from(position));
        let framed = self
            .group
            .panel_groups(&self.config)
            .into_iter()
            .map(|group| {
                let content = match position {
                    Position::Left => container(group).padding([layer.corner_radius.top_left, 0.0]),
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
        "vetrata_readme_showcase"
    }

    fn settings(&self) -> Settings {
        Settings {
            default_font: self.config.theme.font.iced_font,
            antialiasing: true,
            ..Settings::default()
        }
    }

    fn window(&self) -> Option<window::Settings> {
        None
    }

    fn boot(&self) -> (ShowcaseState, Task<Message>) {
        let mut tooltip = TooltipHost::default();
        let _ = tooltip.open_surface(SHOWCASE_OUTPUT);
        let state = ShowcaseState {
            config: self.config.clone(),
            shell_settings: ShellSettings::default(),
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            font_fallbacks: FontFallbacks::default(),
            theme: Theme::from(&self.config.theme.palette),
            group: (self.demo.group)(),
            tooltip,
        };
        (state, Task::none())
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
        stack(once(strip).chain(state.tooltip_layer()))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn theme(&self, state: &ShowcaseState, _window: window::Id) -> Option<Theme> {
        Some(state.theme.clone())
    }

    fn style(&self, state: &ShowcaseState, _theme: &Theme) -> Style {
        let palette = &state.config.theme.palette;
        Style {
            background_color: palette.surface_raised,
            text_color: palette.bar_foreground,
        }
    }
}
