use iced::{
    Element, Length, Settings, Task, Theme,
    alignment::Vertical,
    theme::Style,
    widget::{container, stack},
    window,
};
use iced_test::program::Program;

use crate::{
    config::Config,
    desktop_shell::{
        Message,
        hover_tooltips::{TooltipHost, TooltipMessage},
        monitor_surfaces::{PanelEdge, Position},
    },
    readme_showcase::{scene_script::Scene, showcased_group::ShowcasedGroup},
    state::ShellSettings,
    widgets::{favicon::Favicons, panel_frame::panel_frame, tooltip_body::Tooltip},
};

const SHOWCASE_OUTPUT: &str = "readme-showcase";

pub struct ShowcaseProgram {
    pub scene: &'static Scene,
    pub config: Config,
}

pub struct ShowcaseState {
    config: Config,
    shell_settings: ShellSettings,
    favicons: Favicons,
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

    fn panel_edge() -> PanelEdge {
        PanelEdge {
            output: SHOWCASE_OUTPUT.to_owned(),
            position: Position::Top,
        }
    }

    fn group_strip(&self) -> Option<Element<'_, Message>> {
        let panel = &self.config.layout.panel;
        let frame_style = self.shell_settings.frame_style(&self.config);
        let outlined_sides = self
            .shell_settings
            .outlined_sides(Position::Top.inward_sides());
        self.group.view(&self.config).map(|group| {
            panel_frame(
                container(group)
                    .height(Length::Fixed(panel.height))
                    .align_y(Vertical::Center)
                    .padding([
                        0.0,
                        self.shell_settings.panels.frame_corner_inset() + panel.group_padding,
                    ]),
                &frame_style,
            )
            .outlined_sides(outlined_sides)
            .into()
        })
    }

    fn tooltip_layer(&self) -> Option<Element<'_, Message>> {
        let surface = self.tooltip.surface_on(SHOWCASE_OUTPUT)?;
        self.tooltip.view(
            surface,
            &self.config,
            &self.shell_settings,
            &self.favicons,
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
    type Executor = iced::executor::Default;

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
            favicons: Favicons::default(),
            theme: Theme::from(&self.config.theme.palette),
            group: (self.scene.boot_group)(),
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
                state
                    .tooltip
                    .open(&ShowcaseState::panel_edge(), target, anchor, press_hints);
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
        stack(
            [
                state
                    .group_strip()
                    .map(|strip| container(strip).padding(gap).into()),
                state.tooltip_layer(),
            ]
            .into_iter()
            .flatten(),
        )
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
