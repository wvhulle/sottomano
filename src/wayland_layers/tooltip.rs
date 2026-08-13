use std::{collections::HashMap, slice::from_ref, time::SystemTime};

use iced::{
    Border, Color, Element, Event, Font, Length, Padding, Rectangle, Size, Task, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    font::{Style as FontStyle, Weight},
    widget::{Column, Row as RowWidget, container, row},
    window::Id,
};
use iced_layershell::reexport::{Anchor, Layer, NewLayerShellSettings};

use crate::{
    app::Message,
    config::Config,
    data::source::{RefreshInfo, Row, Tooltip, TooltipBody, level_color, niri::OutputGeometry},
    wayland_layers::surface::{self, Margin, Position, Surface, SurfaceSize},
    widgets::text::{TextStyle, bar_text},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoverTarget {
    Cpu,
    Memory,
    Temperature,
    Disk,
    Battery,
    PowerDraw,
    Floating,
    FloatOffset,
    BlurBackground,
    DisconnectedGroups,
    DarkenChrome,
    JournalSystem,
    JournalUser,
    JournalKernel,
    Bluetooth,
    Ethernet,
    Wifi,
    Mobile,
    AudioSink,
    AudioSource,
    PowerProfile,
    Backlight(String),
    GammaDim,
    ColorTemperature,
    ClightAuto,
    DarkMode,
    IdleLockUser,
    IdleScreenOff,
    IdleStandby,
    IdlePowerDown,
    InternetDown,
    InternetUp,
    Tailscale,
    Wireguard(String),
    WeatherTemp,
    WeatherRain,
    PollenBirch,
    PollenGrass,
    Location,
    Media,
    AiOpenRouter,
    AiClaudeOauth,
    AiClaudeCcusage,
    AiCopilotMetered,
    AiCopilotFree,
    AiCopilotCli,
    Tray(Box<Tooltip>),
    Window(u64),
}

const GAP_PX: i32 = 6;
const PANEL_PADDING: Padding = Padding::new(10.0);
const ROW_SPACING_PX: f32 = 3.0;
const COLUMN_GAP_PX: f32 = 14.0;
const FOOTER_GAP_PX: f32 = 6.0;
const FOOTER_SCALE: f32 = 0.85;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
};
const WIDEST_RELTIME: &str = "59 min ago";

#[derive(Debug, Clone)]
pub enum TooltipMessage {
    EnterAnchor {
        target: HoverTarget,
        anchor: Rectangle,
    },
    Enter {
        surface: Id,
        target: HoverTarget,
        anchor: Rectangle,
    },
    Exit {
        target: HoverTarget,
    },
}

struct Shown {
    surface: Id,
    target: HoverTarget,
    anchor: Rectangle,
    output_width: Option<i32>,
    output: String,
    position: Position,
}

#[derive(Debug, Clone)]
pub struct MenuAnchor {
    pub anchor: Rectangle,
    pub output: String,
    pub output_width: Option<i32>,
    pub position: Position,
}

#[derive(Default)]
pub struct TooltipHost {
    shown: Option<Shown>,
}

impl TooltipHost {
    pub fn update(&mut self, message: TooltipMessage) -> Task<Message> {
        match message {
            TooltipMessage::Enter { .. } | TooltipMessage::EnterAnchor { .. } => Task::none(),
            TooltipMessage::Exit { target } => self.exit(&target),
        }
    }

    pub fn open(
        &mut self,
        parent: &Surface,
        target: HoverTarget,
        anchor: Rectangle,
        outputs: &HashMap<String, OutputGeometry>,
        config: &Config,
        content: &Tooltip,
    ) -> Task<Message> {
        if self.shown.as_ref().is_some_and(|s| s.target == target) {
            return Task::none();
        }
        let output_width = outputs.get(&parent.output).map(|geometry| geometry.width);
        let measured = measure_panel(content, config);
        let clamped = match output_width {
            Some(width) => Size::new(measured.width.min(width.max(0) as f32), measured.height),
            None => measured,
        };
        let size = surface_size(clamped);
        let settings = tooltip_settings(
            &parent.output,
            parent.position,
            anchor,
            output_width,
            size,
            config,
        );
        let surface = Id::unique();
        let close = self.close();
        self.shown = Some(Shown {
            surface,
            target,
            anchor,
            output_width,
            output: parent.output.clone(),
            position: parent.position,
        });
        Task::batch([
            close,
            Task::done(Message::NewLayerShell {
                settings,
                id: surface,
            }),
        ])
    }

    fn exit(&mut self, target: &HoverTarget) -> Task<Message> {
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| &shown.target == target)
        {
            self.close()
        } else {
            Task::none()
        }
    }

    pub fn close(&mut self) -> Task<Message> {
        match self.shown.take() {
            Some(shown) => Task::done(Message::RemoveWindow(shown.surface)),
            None => Task::none(),
        }
    }

    pub fn shown_surface(&self) -> Option<Id> {
        self.shown.as_ref().map(|shown| shown.surface)
    }

    pub fn forget(&mut self, surface: Id) {
        if self.shown_surface() == Some(surface) {
            self.shown = None;
        }
    }

    pub fn target_for(&self, surface: Id) -> Option<&HoverTarget> {
        self.shown
            .as_ref()
            .filter(|shown| shown.surface == surface)
            .map(|shown| &shown.target)
    }

    pub fn target(&self) -> Option<&HoverTarget> {
        self.shown.as_ref().map(|shown| &shown.target)
    }

    pub fn menu_anchor(&self) -> Option<MenuAnchor> {
        let shown = self.shown.as_ref()?;
        matches!(shown.target, HoverTarget::Tray(_)).then(|| MenuAnchor {
            anchor: shown.anchor,
            output: shown.output.clone(),
            output_width: shown.output_width,
            position: shown.position,
        })
    }
}

pub fn panel_surface<'a>(tooltip: &Tooltip, config: &Config) -> Element<'a, Message> {
    panel(tooltip, config)
}

pub fn hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    wrap: impl Fn(TooltipMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        wrap: Box::new(wrap),
    })
}

pub fn on_surface(surface: Id, content: Element<'_, Message>) -> Element<'_, Message> {
    content.map(move |message| match message {
        Message::Tooltip(TooltipMessage::EnterAnchor { target, anchor }) => {
            Message::Tooltip(TooltipMessage::Enter {
                surface,
                target,
                anchor,
            })
        }
        other => other,
    })
}

struct HoverArea<'a, M> {
    content: Element<'a, M>,
    target: HoverTarget,
    wrap: Box<dyn Fn(TooltipMessage) -> M + 'a>,
}

#[derive(Default)]
struct HoverState {
    hovered: bool,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for HoverArea<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HoverState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state: &mut HoverState = tree.state.downcast_mut();
        let anchor = layout.bounds();
        let hovered = cursor.is_over(anchor);
        if hovered != state.hovered {
            state.hovered = hovered;
            shell.publish((self.wrap)(if hovered {
                TooltipMessage::EnterAnchor {
                    target: self.target.clone(),
                    anchor,
                }
            } else {
                TooltipMessage::Exit {
                    target: self.target.clone(),
                }
            }));
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

fn tooltip_grid<'a>(
    body: &TooltipBody,
    config: &Config,
    foreground: Color,
    dim: Color,
    now: SystemTime,
) -> Element<'a, Message> {
    match body {
        TooltipBody::Table(rows) => {
            let text_column = |cells: Vec<String>, color: Color| {
                Column::with_children(cells.into_iter().map(|cell| {
                    container(bar_text(&config.font, cell).color(color))
                        .max_width(config.widgets.tooltip_value_width_px)
                        .into()
                }))
                .spacing(ROW_SPACING_PX)
            };
            let mut columns = vec![
                text_column(rows.iter().map(|entry| entry.key.clone()).collect(), dim).into(),
                text_column(
                    rows.iter().map(|entry| entry.value.clone()).collect(),
                    foreground,
                )
                .into(),
            ];
            if rows.iter().any(|entry| entry.second.is_some()) {
                let second_key = |entry: &Row| {
                    entry
                        .second
                        .as_ref()
                        .map(|(key, _)| key.clone())
                        .unwrap_or_default()
                };
                let second_value = |entry: &Row| {
                    entry
                        .second
                        .as_ref()
                        .map(|(_, value)| value.clone())
                        .unwrap_or_default()
                };
                columns.push(text_column(rows.iter().map(second_key).collect(), dim).into());
                columns
                    .push(text_column(rows.iter().map(second_value).collect(), foreground).into());
            }
            RowWidget::with_children(columns)
                .spacing(COLUMN_GAP_PX)
                .into()
        }
        TooltipBody::Log(entries) => row![
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&config.font, relative(entry.at, now))
                    .color(dim)
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&config.font, entry.message.clone())
                    .color(level_color(config, entry.level))
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
        ]
        .spacing(COLUMN_GAP_PX)
        .into(),
        TooltipBody::Text { title, body } => {
            let bold = Font {
                weight: Weight::Bold,
                ..config.font.iced_font
            };
            let mut column = Column::new().spacing(ROW_SPACING_PX);
            if !title.is_empty() {
                column = column.push(
                    bar_text(&config.font, title.clone())
                        .font(bold)
                        .color(foreground),
                );
            }
            if !body.is_empty() {
                column = column.push(bar_text(&config.font, body.clone()).color(dim));
            }
            column.into()
        }
    }
}

fn footer_text(info: RefreshInfo, now: SystemTime) -> String {
    footer_text_with(&relative(info.last, now), info)
}

fn footer_text_with(reltime: &str, info: RefreshInfo) -> String {
    format!(
        "updated {reltime} · every {}",
        humantime::format_duration(info.every),
    )
}

fn surface_size(size: Size) -> SurfaceSize {
    SurfaceSize {
        width: size.width.ceil() as u32,
        height: size.height.ceil() as u32,
    }
}

fn column<'a>(lines: impl Iterator<Item = &'a str>, style: TextStyle) -> Size {
    capped_column(lines, style, f32::INFINITY)
}

fn capped_column<'a>(
    lines: impl Iterator<Item = &'a str>,
    style: TextStyle,
    max_width: f32,
) -> Size {
    let measured: Vec<Size> = lines
        .map(|line| {
            if line.is_empty() {
                Size::new(0.0, style.line_height())
            } else {
                style.measure_wrapped(line, max_width)
            }
        })
        .collect();
    let width = measured.iter().map(|line| line.width).fold(0.0, f32::max);
    let height = measured.iter().map(|line| line.height).sum::<f32>()
        + measured.len().saturating_sub(1) as f32 * ROW_SPACING_PX;
    Size::new(width, height)
}

fn beside(left: Size, right: Size) -> Size {
    Size::new(
        left.width + COLUMN_GAP_PX + right.width,
        left.height.max(right.height),
    )
}

fn stack(top: Size, bottom: Size, gap: f32) -> Size {
    let gap = if top.height > 0.0 && bottom.height > 0.0 {
        gap
    } else {
        0.0
    };
    Size::new(
        top.width.max(bottom.width),
        top.height + gap + bottom.height,
    )
}

fn widened(column: Size, widest: Size) -> Size {
    Size::new(column.width.max(widest.width), column.height)
}

fn measure_panel(tooltip: &Tooltip, config: &Config) -> Size {
    let base = TextStyle::of(&config.font);
    let now = SystemTime::now();

    let grid = match &tooltip.body {
        TooltipBody::Table(rows) => {
            let cap = config.widgets.tooltip_value_width_px;
            let mut size = beside(
                capped_column(rows.iter().map(|entry| entry.key.as_str()), base, cap),
                capped_column(rows.iter().map(|entry| entry.value.as_str()), base, cap),
            );
            if rows.iter().any(|entry| entry.second.is_some()) {
                size = beside(
                    size,
                    beside(
                        capped_column(
                            rows.iter().map(|entry| {
                                entry.second.as_ref().map_or("", |(key, _)| key.as_str())
                            }),
                            base,
                            cap,
                        ),
                        capped_column(
                            rows.iter().map(|entry| {
                                entry
                                    .second
                                    .as_ref()
                                    .map_or("", |(_, value)| value.as_str())
                            }),
                            base,
                            cap,
                        ),
                    ),
                );
            }
            size
        }
        TooltipBody::Log(entries) => {
            let times: Vec<String> = entries
                .iter()
                .map(|entry| relative(entry.at, now))
                .collect();
            beside(
                widened(
                    column(times.iter().map(String::as_str), base),
                    base.measure(WIDEST_RELTIME),
                ),
                column(entries.iter().map(|entry| entry.message.as_str()), base),
            )
        }
        TooltipBody::Text { title, body } => stack(
            base.bold().measure(title),
            base.measure(body),
            ROW_SPACING_PX,
        ),
    };

    let body = match tooltip.footer {
        Some(info) => {
            let style = base.italic().scaled(FOOTER_SCALE);
            let footer = widened(
                style.measure(&footer_text(info, now)),
                style.measure(&footer_text_with(WIDEST_RELTIME, info)),
            )
            .expand(FOOTER_PADDING);
            stack(grid, footer, FOOTER_GAP_PX)
        }
        None => grid,
    };

    body.expand(PANEL_PADDING)
}

fn panel<'a>(tooltip: &Tooltip, config: &Config) -> Element<'a, Message> {
    let foreground = config.palette.bar_foreground;
    let dim = Color {
        a: 0.7,
        ..config.palette.bar_foreground
    };
    let now = SystemTime::now();
    let grid = tooltip_grid(&tooltip.body, config, foreground, dim, now);

    let italic = Font {
        style: FontStyle::Italic,
        ..config.font.iced_font
    };
    let body = match tooltip.footer {
        Some(info) => {
            let footer = container(
                bar_text(&config.font, footer_text(info, now))
                    .size(config.font.text_size_px * FOOTER_SCALE)
                    .font(italic)
                    .color(dim),
            )
            .padding(FOOTER_PADDING);
            Column::with_children([grid, footer.into()])
                .spacing(FOOTER_GAP_PX)
                .into()
        }
        None => grid,
    };

    let background = Color {
        a: 1.0,
        ..config.palette.bar_background
    };
    let radius = f32::from(config.chrome.corner_radius_px);
    container(body)
        .center(Length::Fill)
        .padding(PANEL_PADDING)
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: radius.into(),
            },
            text_color: Some(foreground),
            ..Default::default()
        })
        .into()
}

pub(crate) fn relative(at: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(at).map_or(0, |d| d.as_secs());
    match secs {
        0..=4 => "just now".to_string(),
        5..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} d ago", secs / 86_400),
    }
}

pub(crate) fn panel_left(anchor_x: f32, output_width: Option<i32>, panel_width: u32) -> i32 {
    let left = anchor_x as i32;
    let left = match output_width {
        Some(width) => left.min(width - panel_width.cast_signed()),
        None => left,
    };
    left.max(0)
}

pub(crate) fn tooltip_margin(position: Position, left: i32, bar: i32) -> Margin {
    match position {
        Position::Top => Margin {
            top: bar + GAP_PX,
            right: 0,
            bottom: 0,
            left,
        },
        Position::Bottom => Margin {
            top: 0,
            right: 0,
            bottom: bar + GAP_PX,
            left,
        },
    }
}

fn tooltip_settings(
    output: &str,
    position: Position,
    anchor: Rectangle,
    output_width: Option<i32>,
    size: SurfaceSize,
    config: &Config,
) -> NewLayerShellSettings {
    let bar = config.chrome.outer_height_px as i32;
    let left = panel_left(anchor.x, output_width, size.width);
    NewLayerShellSettings {
        size: Some(size.dimensions()),
        exclusive_zone: Some(0),
        anchor: position.edge() | Anchor::Left,
        margin: Some(tooltip_margin(position, left, bar).edges()),
        layer: Layer::Overlay,
        events_transparent: true,
        ..surface::base_settings(Some(output), "vetrata-tooltip")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ago(secs: u64) -> String {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        relative(now - Duration::from_secs(secs), now)
    }

    #[test]
    fn relative_time_buckets() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(4), "just now");
        assert_eq!(ago(5), "5 s ago");
        assert_eq!(ago(59), "59 s ago");
        assert_eq!(ago(60), "1 min ago");
        assert_eq!(ago(3599), "59 min ago");
        assert_eq!(ago(3600), "1 h ago");
        assert_eq!(ago(86_399), "23 h ago");
        assert_eq!(ago(86_400), "1 d ago");
    }

    #[test]
    fn future_timestamp_is_just_now() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert_eq!(relative(now + Duration::from_secs(60), now), "just now");
    }
}
