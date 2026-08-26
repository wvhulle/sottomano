use std::time::SystemTime;

use iced::{
    Color, Element, Font, Padding,
    font::{Style as FontStyle, Weight},
    widget::{Column, Row as RowWidget, container, row},
};

use crate::{
    app::Message,
    config::Config,
    data::source::{LogEntry, RefreshInfo, Row, Tooltip, TooltipBody, level_color},
    widget_groups::workspaces_group::workspace_preview::WorkspaceSchematic,
    widgets::{favicon::Favicons, text::bar_text},
};

pub const CONTENT_PADDING: Padding = Padding::new(10.0);
const ROW_SPACING_PX: f32 = 3.0;
const COLUMN_GAP_PX: f32 = 14.0;
const FOOTER_GAP_PX: f32 = 6.0;
const SECTION_GAP_PX: f32 = 8.0;
const DIM_ALPHA: f32 = 0.7;
const FOOTER_SCALE: f32 = 0.85;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
};

pub struct TooltipContent<'a> {
    tooltip: &'a Tooltip,
    config: &'a Config,
    favicons: &'a Favicons,
    opacity: f32,
    now: SystemTime,
}

impl<'a> TooltipContent<'a> {
    pub fn new(tooltip: &'a Tooltip, config: &'a Config, favicons: &'a Favicons) -> Self {
        Self {
            tooltip,
            config,
            favicons,
            opacity: 1.0,
            now: SystemTime::now(),
        }
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn view(self) -> Element<'static, Message> {
        let grid = self.grid();
        let body = match self.tooltip.footer {
            Some(info) => Column::with_children([grid, self.footer(info)])
                .spacing(FOOTER_GAP_PX)
                .into(),
            None => grid,
        };
        container(body).padding(CONTENT_PADDING).into()
    }

    fn foreground(&self) -> Color {
        self.config.palette.bar_foreground.scale_alpha(self.opacity)
    }

    fn dim(&self) -> Color {
        Color {
            a: DIM_ALPHA,
            ..self.config.palette.bar_foreground
        }
        .scale_alpha(self.opacity)
    }

    fn footer(&self, info: RefreshInfo) -> Element<'static, Message> {
        let italic = Font {
            style: FontStyle::Italic,
            ..self.config.font.iced_font
        };
        container(
            bar_text(&self.config.font, footer_text(info, self.now))
                .size(self.config.font.text_size_px * FOOTER_SCALE)
                .font(italic)
                .color(self.dim()),
        )
        .padding(FOOTER_PADDING)
        .into()
    }

    fn grid(&self) -> Element<'static, Message> {
        match &self.tooltip.body {
            TooltipBody::Table(rows) => self.table(rows),
            TooltipBody::Workspace(preview) => Column::with_children([
                self.table(&preview.header),
                WorkspaceSchematic::new(preview, self.config, self.favicons)
                    .opacity(self.opacity)
                    .view(),
            ])
            .spacing(SECTION_GAP_PX)
            .into(),
            TooltipBody::Log(entries) => self.log(entries),
            TooltipBody::Text { title, body } => self.text(title, body),
        }
    }

    fn log(&self, entries: &[LogEntry]) -> Element<'static, Message> {
        row![
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&self.config.font, relative(entry.at, self.now))
                    .color(self.dim())
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&self.config.font, entry.message.clone())
                    .color(level_color(self.config, entry.level).scale_alpha(self.opacity))
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
        ]
        .spacing(COLUMN_GAP_PX)
        .into()
    }

    fn text(&self, title: &str, body: &str) -> Element<'static, Message> {
        let bold = Font {
            weight: Weight::Bold,
            ..self.config.font.iced_font
        };
        let mut column = Column::new().spacing(ROW_SPACING_PX);
        if !title.is_empty() {
            column = column.push(
                bar_text(&self.config.font, title.to_owned())
                    .font(bold)
                    .color(self.foreground()),
            );
        }
        if !body.is_empty() {
            column = column.push(bar_text(&self.config.font, body.to_owned()).color(self.dim()));
        }
        column.into()
    }

    fn table(&self, rows: &[Row]) -> Element<'static, Message> {
        let mut columns = vec![
            self.text_column(rows.iter().map(|entry| entry.key.clone()), self.dim()),
            self.text_column(
                rows.iter().map(|entry| entry.value.clone()),
                self.foreground(),
            ),
        ];
        if rows.iter().any(|entry| entry.second.is_some()) {
            let part = |entry: &Row, second: bool| {
                entry
                    .second
                    .as_ref()
                    .map_or_else(String::new, |(key, value)| {
                        if second { value.clone() } else { key.clone() }
                    })
            };
            columns.push(self.text_column(rows.iter().map(|e| part(e, false)), self.dim()));
            columns.push(self.text_column(rows.iter().map(|e| part(e, true)), self.foreground()));
        }
        RowWidget::with_children(columns)
            .spacing(COLUMN_GAP_PX)
            .into()
    }

    fn text_column(
        &self,
        cells: impl Iterator<Item = String>,
        color: Color,
    ) -> Element<'static, Message> {
        Column::with_children(cells.map(|cell| {
            container(bar_text(&self.config.font, cell).color(color))
                .max_width(self.config.widgets.tooltip_value_width_px)
                .into()
        }))
        .spacing(ROW_SPACING_PX)
        .into()
    }
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
