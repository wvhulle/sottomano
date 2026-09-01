use std::time::SystemTime;

use iced::{
    Color, Element, Font, Padding,
    alignment::{Horizontal, Vertical},
    font::{Style as FontStyle, Weight},
    widget::{Column, Container, Space, container, table},
};

use crate::{
    app::Message,
    config::{Config, font::FontConfig},
    data::source::{
        LogEntry, RefreshInfo, Row, Section, Tooltip, TooltipBody, TrayMenuBody, level_color,
    },
    widget_groups::{
        tray_group::TrayMessage, workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        context_menu::context_menu, copyable::copyable, favicon::Favicons,
        section_column::section_column, text::bar_text,
    },
};

pub const CONTENT_PADDING: Padding = Padding::new(10.0);
const ROW_SPACING_PX: f32 = 3.0;
const COLUMN_GAP_PX: f32 = 14.0;
const FOOTER_GAP_PX: f32 = 6.0;
const SECTION_GAP_PX: f32 = 8.0;
const TRAY_MENU_WIDTH_PX: f32 = 250.0;
const DIVIDER_THICKNESS_PX: f32 = 1.0;
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

    fn flash(&self) -> Color {
        self.config.palette.focus.scale_alpha(self.opacity)
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
            TooltipBody::Sections(sections) => self.sections(sections),
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
            TooltipBody::TrayMenu(menu) => self.tray_menu(menu),
        }
    }

    fn tray_menu(&self, menu: &TrayMenuBody) -> Element<'static, Message> {
        let address = menu.address.clone();
        let menu_path = menu.menu_path.clone();
        let entries = context_menu(
            self.config,
            &menu.nodes,
            &menu.open_submenus,
            &move |id| {
                Message::Tray(TrayMessage::MenuItemClicked {
                    address: address.clone(),
                    menu_path: menu_path.clone(),
                    id,
                })
            },
            &|id| Message::Tray(TrayMessage::ToggleSubmenu(id)),
            CONTENT_PADDING.left,
        );
        let mut column = Column::new().spacing(SECTION_GAP_PX);
        if !menu.title.is_empty() || !menu.body.is_empty() {
            column = column.push(self.text(&menu.title, &menu.body));
        }
        column
            .push(container(entries).width(TRAY_MENU_WIDTH_PX))
            .into()
    }

    fn log(&self, entries: &[LogEntry]) -> Element<'static, Message> {
        let dim = self.dim();
        let flash = self.flash();
        let opacity = self.opacity;
        let key_font = self.config.font.clone();
        let value_font = self.config.font.clone();
        let value_width = self.config.widgets.tooltip_value_width_px;
        let config = self.config;
        let now = self.now;

        key_value_table(
            entries.iter().cloned(),
            move |entry: LogEntry| {
                cell(&key_font, relative(entry.at, now), dim, value_width).into()
            },
            move |entry: LogEntry| {
                let color = level_color(config, entry.level).scale_alpha(opacity);
                let text = entry.message;
                let value = cell(&value_font, text.clone(), color, value_width);
                copyable(value, text, Message::CopyText, flash)
            },
        )
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

    fn sections(&self, sections: &[Section]) -> Element<'static, Message> {
        let italic = Font {
            style: FontStyle::Italic,
            ..self.config.font.iced_font
        };
        let heading = self.config.palette.inactive.scale_alpha(self.opacity);
        let divider = self.config.palette.surface_raised.scale_alpha(self.opacity);
        let font = self.config.font.clone();

        let filled = sections.iter().filter(|section| !section.rows.is_empty());
        let mut column = Column::new().spacing(SECTION_GAP_PX);
        for section in filled {
            let mut group = Column::new().spacing(ROW_SPACING_PX);
            if let Some(title) = &section.title {
                group = group.push(bar_text(&font, title.clone()).font(italic).color(heading));
            }
            column = column.push(group.push(self.table(&section.rows)));
        }
        section_column(column, divider, DIVIDER_THICKNESS_PX, CONTENT_PADDING.left)
    }

    fn table(&self, rows: &[Row]) -> Element<'static, Message> {
        let dim = self.dim();
        let foreground = self.foreground();
        let flash = self.flash();
        let key_font = self.config.font.clone();
        let value_font = self.config.font.clone();
        let value_width = self.config.widgets.tooltip_value_width_px;

        key_value_table(
            rows.iter().cloned(),
            move |entry: Row| cell(&key_font, entry.key, dim, value_width).into(),
            move |entry: Row| {
                let text = entry.value;
                let value = cell(&value_font, text.clone(), foreground, value_width);
                if text.trim().is_empty() {
                    return value.into();
                }
                copyable(value, text, Message::CopyText, flash)
            },
        )
    }
}

fn key_value_table<'b, T: Clone + 'static>(
    rows: impl IntoIterator<Item = T>,
    key: impl Fn(T) -> Element<'static, Message> + 'b,
    value: impl Fn(T) -> Element<'static, Message> + 'b,
) -> Element<'static, Message> {
    table::table(
        [
            table::column(Space::new(), key)
                .align_x(Horizontal::Right)
                .align_y(Vertical::Top),
            table::column(Space::new(), value).align_y(Vertical::Top),
        ],
        rows,
    )
    .separator(0.0)
    .padding_x(COLUMN_GAP_PX / 2.0)
    .padding_y(ROW_SPACING_PX / 2.0)
    .into()
}

fn cell(
    font: &FontConfig,
    text: String,
    color: Color,
    max_width: f32,
) -> Container<'static, Message> {
    container(bar_text(font, text).color(color)).max_width(max_width)
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
