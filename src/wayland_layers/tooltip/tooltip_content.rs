use std::time::SystemTime;

use iced::{
    Color, Element, Font, Padding,
    font::{Style as FontStyle, Weight},
    widget::{Column, Row as RowWidget, container, row},
};

use crate::{
    app::Message,
    config::Config,
    data::source::{RefreshInfo, Row, Tooltip, TooltipBody, level_color},
    widget_groups::workspaces_group::workspace_preview,
    widgets::text::bar_text,
};

pub const CONTENT_PADDING: Padding = Padding::new(10.0);
const ROW_SPACING_PX: f32 = 3.0;
const COLUMN_GAP_PX: f32 = 14.0;
const FOOTER_GAP_PX: f32 = 6.0;
const SECTION_GAP_PX: f32 = 8.0;
const FOOTER_SCALE: f32 = 0.85;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
};

pub fn render<'a>(tooltip: &Tooltip, config: &Config, alpha: f32) -> Element<'a, Message> {
    let foreground = config.palette.bar_foreground.scale_alpha(alpha);
    let dim = Color {
        a: 0.7,
        ..config.palette.bar_foreground
    }
    .scale_alpha(alpha);
    let now = SystemTime::now();
    let grid = tooltip_grid(&tooltip.body, config, foreground, dim, alpha, now);

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

    container(body).padding(CONTENT_PADDING).into()
}

fn tooltip_grid<'a>(
    body: &TooltipBody,
    config: &Config,
    foreground: Color,
    dim: Color,
    alpha: f32,
    now: SystemTime,
) -> Element<'a, Message> {
    match body {
        TooltipBody::Table(rows) => table_grid(rows, config, foreground, dim),
        TooltipBody::Workspace(preview) => Column::with_children([
            table_grid(&preview.header, config, foreground, dim),
            workspace_preview::schematic(preview, config, foreground, dim, alpha),
        ])
        .spacing(SECTION_GAP_PX)
        .into(),
        TooltipBody::Log(entries) => row![
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&config.font, relative(entry.at, now))
                    .color(dim)
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&config.font, entry.message.clone())
                    .color(level_color(config, entry.level).scale_alpha(alpha))
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

fn table_grid<'a>(
    rows: &[Row],
    config: &Config,
    foreground: Color,
    dim: Color,
) -> Element<'a, Message> {
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
        columns.push(text_column(rows.iter().map(second_value).collect(), foreground).into());
    }
    RowWidget::with_children(columns)
        .spacing(COLUMN_GAP_PX)
        .into()
}

fn footer_text(info: RefreshInfo, now: SystemTime) -> String {
    format!(
        "updated {} · every {}",
        relative(info.last, now),
        humantime::format_duration(info.every),
    )
}

pub fn relative(at: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(at).map_or(0, |d| d.as_secs());
    match secs {
        0..=4 => "just now".to_string(),
        5..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} d ago", secs / 86_400),
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
