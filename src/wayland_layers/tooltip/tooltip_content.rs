use std::{
    iter::once,
    time::{Duration, SystemTime},
};

use chrono::Locale;
use iced::{
    Color, Element, Font, Padding,
    alignment::{Horizontal, Vertical},
    font::{Style as FontStyle, Weight},
    widget::{Column, Container, Space, container, table},
};

use crate::{
    app::Message,
    config::{Config, font::FontConfig},
    widget_groups::{
        tray_group::TrayMessage, window_group::process_tree::ProcessTree,
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        bar_widget::Glyph,
        context_menu::context_menu,
        copyable::{copyable, openable},
        favicon::Favicons,
        section_column::section_column,
        text::{bar_text, column_heading},
        tooltip_body::{
            Cell, RefreshInfo, Row, Section, SectionBody, Tooltip, TooltipBody, TrayMenuBody,
        },
    },
};

const CONTENT_INSET_PX: f32 = 10.0;
const CONTENT_INSET: Padding = Padding {
    top: 0.0,
    right: CONTENT_INSET_PX,
    bottom: 0.0,
    left: CONTENT_INSET_PX,
};
const SHELL_PADDING: Padding = Padding {
    top: CONTENT_INSET_PX,
    right: 0.0,
    bottom: CONTENT_INSET_PX,
    left: 0.0,
};
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
    right: 6.0 + CONTENT_INSET_PX,
    bottom: 2.0,
    left: 6.0 + CONTENT_INSET_PX,
};

pub struct TooltipContent<'a> {
    tooltip: &'a Tooltip,
    config: &'a Config,
    favicons: &'a Favicons,
    now: SystemTime,
}

impl<'a> TooltipContent<'a> {
    pub fn new(tooltip: &'a Tooltip, config: &'a Config, favicons: &'a Favicons) -> Self {
        Self {
            tooltip,
            config,
            favicons,
            now: SystemTime::now(),
        }
    }

    pub fn view(self) -> Element<'static, Message> {
        let grid = self.grid();
        let body = match self.tooltip.footer {
            Some(info) => Column::with_children([grid, self.footer(info)])
                .spacing(FOOTER_GAP_PX)
                .into(),
            None => grid,
        };
        container(body).padding(SHELL_PADDING).into()
    }

    fn foreground(&self) -> Color {
        self.config.palette.bar_foreground
    }

    fn flash(&self) -> Color {
        self.config.palette.success
    }

    fn dim(&self) -> Color {
        Color {
            a: DIM_ALPHA,
            ..self.config.palette.bar_foreground
        }
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
            TooltipBody::Workspace(preview) => inset(
                Column::with_children([
                    self.table(&preview.header),
                    WorkspaceSchematic::new(preview, self.config, self.favicons).view(),
                ])
                .spacing(SECTION_GAP_PX)
                .into(),
            ),
            TooltipBody::Text { title, body } => inset(self.text(title, body)),
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
        );
        let mut column = Column::new().spacing(SECTION_GAP_PX);
        if !menu.title.is_empty() || !menu.body.is_empty() {
            column = column.push(
                container(self.text(&menu.title, &menu.body))
                    .padding(CONTENT_INSET)
                    .width(TRAY_MENU_WIDTH_PX),
            );
        }
        column
            .push(container(entries).width(TRAY_MENU_WIDTH_PX))
            .into()
    }

    fn text(&self, title: &str, body: &str) -> Element<'static, Message> {
        let bold = Font {
            weight: Weight::Bold,
            ..self.config.font.iced_font
        };
        let max_width = self.config.widgets.tooltip_value_width_px;
        let mut column = Column::new().spacing(ROW_SPACING_PX);
        if !title.is_empty() {
            column = column.push(
                container(
                    bar_text(&self.config.font, title.to_owned())
                        .font(bold)
                        .color(self.foreground()),
                )
                .max_width(max_width),
            );
        }
        if !body.is_empty() {
            column = column.push(cell(
                &self.config.font,
                body.to_owned(),
                self.dim(),
                max_width,
            ));
        }
        column.into()
    }

    fn sections(&self, sections: &[Section]) -> Element<'static, Message> {
        let italic = Font {
            style: FontStyle::Italic,
            ..self.config.font.iced_font
        };
        let heading = self.config.palette.inactive;
        let divider = self.config.palette.surface_raised;
        let font = self.config.font.clone();

        let filled = sections.iter().filter(|section| !section.body.is_empty());
        let mut column = Column::new().spacing(SECTION_GAP_PX);
        for section in filled {
            let mut group = Column::new().spacing(ROW_SPACING_PX);
            if let Some(title) = &section.title {
                group = group.push(bar_text(&font, title.clone()).font(italic).color(heading));
            }
            column = column.push(inset(group.push(self.body(&section.body)).into()));
        }
        section_column(column, divider, DIVIDER_THICKNESS_PX)
    }

    fn body(&self, body: &SectionBody) -> Element<'static, Message> {
        match body {
            SectionBody::Rows(rows) => self.table(rows),
            SectionBody::Columns { headings, rows } => self.headed_table(headings, rows),
            SectionBody::Tree { root, folding } => {
                ProcessTree::new(root, folding, self.config).view()
            }
        }
    }

    fn cell_view(&self) -> CellView {
        CellView {
            font: self.config.font.clone(),
            max_width: self.config.widgets.tooltip_value_width_px,
            locale: self.config.locale,
            flash: self.flash(),
            link: self.config.palette.info,
        }
    }

    fn table(&self, rows: &[Row]) -> Element<'static, Message> {
        let dim = self.dim();
        let cell_view = self.cell_view();
        let widest = rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);

        key_value_table(
            rows.iter().cloned(),
            move |entry: Row| cell_view.key(entry.key, dim),
            (0..widest).map(|position| self.cell_column(position, Space::new().into())),
        )
    }

    fn headed_table(&self, headings: &[String], rows: &[Row]) -> Element<'static, Message> {
        let dim = self.dim();
        let cell_view = self.cell_view();
        let widest = rows
            .iter()
            .map(|row| row.cells.len())
            .chain(once(headings.len()))
            .max()
            .unwrap_or(0);
        let heading_color = self.config.palette.inactive;

        key_value_table(
            rows.iter().cloned(),
            move |entry: Row| cell_view.key(entry.key, dim),
            (0..widest).map(|position| {
                let heading = headings.get(position).cloned().unwrap_or_default();
                self.cell_column(
                    position,
                    column_heading(&self.config.font, heading, heading_color).into(),
                )
                .align_x(Horizontal::Right)
            }),
        )
    }

    fn cell_column(
        &self,
        position: usize,
        heading: Element<'static, Message>,
    ) -> table::Column<'static, 'static, Row, Message> {
        let palette = self.config.palette.clone();
        let cell_view = self.cell_view();

        value_column(heading, move |mut entry: Row| {
            if position >= entry.cells.len() {
                return Space::new().into();
            }
            let foreground = entry.level.color(&palette);
            cell_view.value(entry.cells.swap_remove(position), foreground)
        })
    }
}

struct CellView {
    font: FontConfig,
    max_width: f32,
    locale: Locale,
    flash: Color,
    link: Color,
}

impl CellView {
    fn key(&self, key: Cell, color: Color) -> Element<'static, Message> {
        match key {
            Cell::Glyph(icon) => self.glyph(icon, color),
            Cell::Text(text) | Cell::Link(text) => {
                cell(&self.font, text, color, self.max_width).into()
            }
            Cell::Time { at, part } => cell(
                &self.font,
                part.format(&at, self.locale),
                color,
                self.max_width,
            )
            .into(),
        }
    }

    fn value(&self, value: Cell, color: Color) -> Element<'static, Message> {
        match value {
            Cell::Glyph(icon) => self.glyph(icon, color),
            Cell::Text(text) => self.copyable_text(text, color),
            Cell::Time { at, part } => self.copyable_text(part.format(&at, self.locale), color),
            Cell::Link(url) => self.openable_link(url),
        }
    }

    fn openable_link(&self, url: String) -> Element<'static, Message> {
        let value = cell(&self.font, url.clone(), self.link, self.max_width);
        openable(value, url, Message::OpenUrl, self.link)
    }

    fn glyph(&self, icon: Glyph, color: Color) -> Element<'static, Message> {
        icon.view(self.font.text_size_px, color)
    }

    fn copyable_text(&self, text: String, color: Color) -> Element<'static, Message> {
        let value = cell(&self.font, text.clone(), color, self.max_width);
        if text.trim().is_empty() {
            return value.into();
        }
        copyable(value, text, Message::CopyText, self.flash)
    }
}

fn key_value_table<'b, T: Clone + 'static>(
    rows: impl IntoIterator<Item = T>,
    key: impl Fn(T) -> Element<'static, Message> + 'b,
    values: impl IntoIterator<Item = table::Column<'static, 'b, T, Message>>,
) -> Element<'static, Message> {
    let key_column = table::column(Space::new(), key)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top);
    table::table(once(key_column).chain(values), rows)
        .separator(0.0)
        .padding_x(COLUMN_GAP_PX / 2.0)
        .padding_y(ROW_SPACING_PX / 2.0)
        .into()
}

fn value_column<'b, T: 'static>(
    heading: Element<'static, Message>,
    view: impl Fn(T) -> Element<'static, Message> + 'b,
) -> table::Column<'static, 'b, T, Message> {
    table::column(heading, view).align_y(Vertical::Top)
}

fn cell(
    font: &FontConfig,
    text: String,
    color: Color,
    max_width: f32,
) -> Container<'static, Message> {
    container(bar_text(font, text).color(color)).max_width(max_width)
}

fn inset(content: Element<'static, Message>) -> Element<'static, Message> {
    container(content).padding(CONTENT_INSET).into()
}

fn footer_text(info: RefreshInfo, now: SystemTime) -> String {
    format!(
        "updated {} · every {}",
        relative(info.last, now),
        humantime::format_duration(info.every),
    )
}

pub fn relative(at: SystemTime, now: SystemTime) -> String {
    let whole_seconds = now.duration_since(at).map_or(0, |age| age.as_secs());
    format!(
        "{} ago",
        humantime::format_duration(Duration::from_secs(whole_seconds))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_drops_subsecond_precision() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let at = now - Duration::from_millis(3_723_400);
        assert_eq!(relative(at, now), "1h 2m 3s ago");
    }

    #[test]
    fn future_timestamp_is_zero_seconds_ago() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert_eq!(relative(now + Duration::from_secs(60), now), "0s ago");
    }
}
