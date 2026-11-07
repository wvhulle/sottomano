use iced::{
    Background, Border, Color, Element, Length, Padding, Theme,
    alignment::Vertical,
    border::Radius,
    widget::{Row, button, space, text, text::Shaping},
};

use crate::{
    activity_line::activity_line,
    font_faces::TextFace,
    list_end_corners::ListEndCorners,
    shared_metrics::OUTLINE_WIDTH_PX,
    theme_colors::{
        background_strong_color, background_weak_color, secondary_text_color, text_color,
    },
    uniform_width_column::uniform_width_column,
};

pub enum ContextMenuTrailing {
    Nothing,
    Note(String),
    Submenu { open: bool },
}

pub enum ContextMenuRowPress<Message> {
    Enabled(Message),
    Disabled,
}

pub struct ContextMenuRow<'a, Message> {
    pub leading: Option<Element<'a, Message>>,
    pub label: Option<String>,
    pub trailing: ContextMenuTrailing,
    pub depth: usize,
    pub press: ContextMenuRowPress<Message>,
}

pub enum ContextMenuLine<'a, Message> {
    Row(ContextMenuRow<'a, Message>),
    Divider,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContextMenuMetrics {
    pub face: TextFace,
    pub note_size_px: f32,
    pub icon_size_px: f32,
    pub item_gap_px: f32,
    pub surface_inset_px: f32,
}

impl Default for ContextMenuMetrics {
    fn default() -> Self {
        Self {
            face: TextFace::default(),
            note_size_px: 12.0,
            icon_size_px: 14.0,
            item_gap_px: 4.0,
            surface_inset_px: 8.0,
        }
    }
}

type SubmenuChevron<'a, Message> =
    Box<dyn Fn(bool, fn(&Theme) -> Color) -> Element<'a, Message> + 'a>;

pub struct ContextMenu<'a, Message> {
    lines: Vec<ContextMenuLine<'a, Message>>,
    corners: Radius,
    metrics: ContextMenuMetrics,
    submenu_chevron: SubmenuChevron<'a, Message>,
}

pub fn context_menu<'a, Message: Clone + 'a>(
    lines: impl IntoIterator<Item = ContextMenuLine<'a, Message>>,
) -> ContextMenu<'a, Message> {
    ContextMenu {
        lines: lines.into_iter().collect(),
        corners: Radius::default(),
        metrics: ContextMenuMetrics::default(),
        submenu_chevron: Box::new(|open, color| {
            text(if open { "\u{25be}" } else { "\u{25b8}" })
                .shaping(Shaping::Advanced)
                .style(move |theme| text::Style {
                    color: Some(color(theme)),
                })
                .into()
        }),
    }
}

impl<'a, Message> ContextMenu<'a, Message> {
    #[must_use]
    pub fn corners(self, corners: Radius) -> Self {
        Self { corners, ..self }
    }

    #[must_use]
    pub fn metrics(self, metrics: ContextMenuMetrics) -> Self {
        Self { metrics, ..self }
    }

    #[must_use]
    pub fn submenu_chevron(
        self,
        submenu_chevron: impl Fn(bool, fn(&Theme) -> Color) -> Element<'a, Message> + 'a,
    ) -> Self {
        Self {
            submenu_chevron: Box::new(submenu_chevron),
            ..self
        }
    }
}

impl<'a, Message: Clone + 'a> From<ContextMenu<'a, Message>> for Element<'a, Message> {
    fn from(menu: ContextMenu<'a, Message>) -> Self {
        let ContextMenu {
            lines,
            corners,
            metrics,
            submenu_chevron,
        } = menu;
        let kept_lines = without_redundant_dividers(lines);
        let end_corners = ListEndCorners::new(kept_lines.len(), corners);
        uniform_width_column(kept_lines.into_iter().enumerate().map(|(position, line)| {
            match line {
                ContextMenuLine::Row(row) => entry(
                    &metrics,
                    &*submenu_chevron,
                    row,
                    end_corners.radius_at(position),
                ),
                ContextMenuLine::Divider => divider(),
            }
        }))
        .spacing(metrics.item_gap_px)
        .into()
    }
}

fn entry<'a, Message: Clone + 'a>(
    style: &ContextMenuMetrics,
    submenu_chevron: &(dyn Fn(bool, fn(&Theme) -> Color) -> Element<'a, Message> + 'a),
    row: ContextMenuRow<'a, Message>,
    highlight_radius: Radius,
) -> Element<'a, Message> {
    let icon_size = style.icon_size_px;
    let row_spacing = style.item_gap_px;
    let color: fn(&Theme) -> Color = match row.press {
        ContextMenuRowPress::Enabled(_) => text_color,
        ContextMenuRowPress::Disabled => secondary_text_color,
    };

    let mut content = Row::new()
        .spacing(row_spacing)
        .align_y(Vertical::Center)
        .width(Length::Fill);
    if let Some(element) = row.leading {
        content = content.push(element);
    }
    if let Some(label) = row.label {
        content = content.push(style.face.text(label).style(move |theme| text::Style {
            color: Some(color(theme)),
        }));
    }
    match row.trailing {
        ContextMenuTrailing::Nothing => {}
        ContextMenuTrailing::Note(note) => {
            content = content.push(space::horizontal()).push(
                style
                    .face
                    .text(note)
                    .size(style.note_size_px)
                    .style(|theme| text::Style {
                        color: Some(background_strong_color(theme)),
                    }),
            );
        }
        ContextMenuTrailing::Submenu { open } => {
            content = content
                .push(space::horizontal())
                .push(submenu_chevron(open, color));
        }
    }

    let glyph_column = icon_size + row_spacing;
    let inset = style.surface_inset_px;
    let padding = Padding {
        top: row_spacing,
        right: inset,
        bottom: row_spacing,
        left: inset + row.depth as f32 * glyph_column,
    };
    let styled = button(content)
        .width(Length::Fill)
        .padding(padding)
        .style(move |theme, status| button_style(theme, status, color, highlight_radius));
    match row.press {
        ContextMenuRowPress::Enabled(message) => styled.on_press(message).into(),
        ContextMenuRowPress::Disabled => styled.into(),
    }
}

fn button_style(
    theme: &Theme,
    status: button::Status,
    text_role: fn(&Theme) -> Color,
    highlight_radius: Radius,
) -> button::Style {
    let background = matches!(status, button::Status::Hovered | button::Status::Pressed)
        .then(|| Background::Color(background_weak_color(theme)));
    button::Style {
        background,
        text_color: text_role(theme),
        border: Border {
            radius: highlight_radius,
            ..Border::default()
        },
        ..Default::default()
    }
}

fn divider<'a, Message: 'a>() -> Element<'a, Message> {
    activity_line(background_weak_color, 1.0, OUTLINE_WIDTH_PX)
}

fn without_redundant_dividers<Message>(
    lines: Vec<ContextMenuLine<'_, Message>>,
) -> Vec<ContextMenuLine<'_, Message>> {
    let mut kept = lines.into_iter().fold(Vec::new(), |mut kept, line| {
        let leading_divider = matches!(line, ContextMenuLine::Divider)
            && kept
                .last()
                .is_none_or(|last| matches!(last, ContextMenuLine::Divider));
        if !leading_divider {
            kept.push(line);
        }
        kept
    });
    if kept
        .last()
        .is_some_and(|last| matches!(last, ContextMenuLine::Divider))
    {
        kept.pop();
    }
    kept
}

#[cfg(test)]
mod tests {
    use iced::{
        Point, Settings, Size,
        mouse::{Button, Event as MouseEvent},
        widget::text,
    };
    use iced_test::Simulator;

    use super::*;

    const SHORT_LABEL: &str = "Cairn";
    const WIDE_SIBLING: &str = "a sibling row far wider than any label in the menu above it";

    #[test]
    fn a_menu_inside_a_wider_card_spans_the_card_width() {
        let menu_rows = [
            ContextMenuLine::Row(ContextMenuRow {
                leading: None,
                label: Some(SHORT_LABEL.to_owned()),
                trailing: ContextMenuTrailing::Nothing,
                depth: 0,
                press: ContextMenuRowPress::Enabled(()),
            }),
            ContextMenuLine::Divider,
            ContextMenuLine::Row(ContextMenuRow {
                leading: None,
                label: Some("Store".to_owned()),
                trailing: ContextMenuTrailing::Nothing,
                depth: 0,
                press: ContextMenuRowPress::Disabled,
            }),
        ];
        let card: Element<'_, ()> = uniform_width_column([
            Element::from(context_menu(menu_rows)),
            text(WIDE_SIBLING).into(),
        ])
        .into();
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(1200.0, 400.0), card);
        let short_label = simulator
            .find(SHORT_LABEL)
            .expect("the short label is laid out")
            .bounds();
        let wide_sibling = simulator
            .find(WIDE_SIBLING)
            .expect("the wide sibling is laid out")
            .bounds();
        let near_card_right_edge = Point::new(
            wide_sibling.x + wide_sibling.width - 2.0,
            short_label.center_y(),
        );
        simulator.point_at(near_card_right_edge);
        let _moved = simulator.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
            position: near_card_right_edge,
        })]);
        let _pressed_and_released = simulator.simulate([
            iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left)),
            iced::Event::Mouse(MouseEvent::ButtonReleased(Button::Left)),
        ]);
        assert_eq!(simulator.into_messages().count(), 1);
    }
}
