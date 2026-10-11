use std::{
    iter::once,
    time::{Duration, SystemTime},
};

use chrono::Locale;
use iced::{
    Color, Element, Padding,
    alignment::{Horizontal, Vertical},
    border::Radius,
    widget::{Column, Container, Space, Text, container, row, table},
};

use crate::{
    config::{Config, theme::font::ResolvedFont},
    desktop_shell::{Message, hover_tooltips::GestureHint},
    widget_groups::{
        tray_group::TrayMessage, window_group::process_tree::ProcessTree,
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        bar_widget::Glyph,
        context_menu::context_menu,
        copyable::{copyable, openable},
        favicon::Favicons,
        loading_text::{LoadingPhase, loading_text},
        section_column::section_column,
        text::{bar_text, column_heading},
        tooltip_body::{
            Cell, KeyValue, RecordCells, RefreshInfo, Section, SectionBody, Tooltip, TooltipBody,
            TrayMenuBody,
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
const SECTION_SEPARATION_PX: f32 = 18.0;
const DIVIDER_THICKNESS_PX: f32 = 1.0;
const DIM_ALPHA: f32 = 0.7;
const GESTURE_ICON_GAP_PX: f32 = 4.0;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 6.0 + CONTENT_INSET_PX,
    bottom: 2.0,
    left: 6.0 + CONTENT_INSET_PX,
};

#[derive(Clone, Copy)]
pub struct TooltipContent<'a> {
    tooltip: &'a Tooltip,
    config: &'a Config,
    corner_radius: Radius,
    favicons: &'a Favicons,
    now: SystemTime,
    phase: Option<LoadingPhase>,
}

impl<'a> TooltipContent<'a> {
    pub fn new(
        tooltip: &'a Tooltip,
        config: &'a Config,
        corner_radius: Radius,
        favicons: &'a Favicons,
    ) -> Self {
        Self {
            tooltip,
            config,
            corner_radius,
            favicons,
            now: SystemTime::now(),
            phase: None,
        }
    }

    fn in_phase(self, phase: LoadingPhase) -> Self {
        Self {
            phase: Some(phase),
            ..self
        }
    }

    pub fn view(self) -> Element<'static, Message> {
        let gestures = self.tooltip.press_hints.gestures();
        let press_hint = (!gestures.is_empty()).then(|| self.gesture_hints(&gestures));
        let refresh = self
            .tooltip
            .footer
            .map(|info| self.footer(footer_text(info, self.now)));
        let body =
            Column::with_children(press_hint.into_iter().chain([self.grid()]).chain(refresh))
                .spacing(FOOTER_GAP_PX);
        container(body).padding(SHELL_PADDING).into()
    }

    fn gesture_hints(&self, gestures: &[GestureHint]) -> Element<'static, Message> {
        let icon_size = self.config.theme.font.small_text_size_px;
        let dim = self.dim();
        container(
            row(gestures.iter().map(|hint| {
                row([
                    Glyph::from(hint.gesture.glyph()).view(icon_size, dim),
                    self.annotation(hint.does.to_owned()).into(),
                ])
                .spacing(GESTURE_ICON_GAP_PX)
                .align_y(Vertical::Center)
                .into()
            }))
            .spacing(COLUMN_GAP_PX),
        )
        .padding(FOOTER_PADDING)
        .into()
    }

    fn foreground(&self) -> Color {
        self.config.theme.palette.bar_foreground
    }

    fn flash(&self) -> Color {
        self.config.theme.palette.success
    }

    fn dim(&self) -> Color {
        Color {
            a: DIM_ALPHA,
            ..self.config.theme.palette.bar_foreground
        }
    }

    fn footer(&self, text: String) -> Element<'static, Message> {
        container(self.annotation(text))
            .padding(FOOTER_PADDING)
            .into()
    }

    fn small_dim_text(&self, text: String) -> Text<'static> {
        bar_text(&self.config.theme.font, text)
            .size(self.config.theme.font.small_text_size_px)
            .color(self.dim())
    }

    fn annotation(&self, text: String) -> Text<'static> {
        self.small_dim_text(text)
            .font(self.config.theme.font.italic())
    }

    fn grid(&self) -> Element<'static, Message> {
        match &self.tooltip.body {
            TooltipBody::Sections(sections) => self.sections(sections),
            TooltipBody::Workspace(preview) => inset(
                WorkspaceSchematic::new(preview, self.config, self.corner_radius, self.favicons)
                    .view(),
            ),
            TooltipBody::Text { title, body } => inset(self.text(title, body.as_deref())),
            TooltipBody::TrayMenu(menu) => self.tray_menu(menu),
        }
    }

    fn tray_menu(&self, menu: &TrayMenuBody) -> Element<'static, Message> {
        let address = menu.address.clone();
        let menu_path = menu.menu_path.clone();
        let entries = context_menu(
            self.config,
            self.corner_radius,
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
        Column::new()
            .spacing(SECTION_GAP_PX)
            .push(
                container(self.text(&menu.title, menu.body.as_deref()))
                    .padding(CONTENT_INSET)
                    .width(TRAY_MENU_WIDTH_PX),
            )
            .push(container(entries).width(TRAY_MENU_WIDTH_PX))
            .into()
    }

    fn bold_title(&self, title: &str) -> Text<'static> {
        bar_text(&self.config.theme.font, title.to_owned())
            .font(self.config.theme.font.bold())
            .color(self.foreground())
    }

    fn text(&self, title: &str, body: Option<&str>) -> Element<'static, Message> {
        let max_width = self.config.tooltips.value_width_px;
        let title = container(self.bold_title(title)).max_width(max_width);
        let body = body.map(|body| {
            cell(
                &self.config.theme.font,
                body.to_owned(),
                self.dim(),
                max_width,
            )
        });

        Column::with_children(once(title).chain(body).map(Element::from))
            .spacing(ROW_SPACING_PX)
            .into()
    }

    fn sections(&self, sections: &[Section]) -> Element<'static, Message> {
        let filled = sections
            .iter()
            .filter(|section| !section.body().is_empty())
            .map(|section| self.section(section));

        section_column(
            Column::with_children(filled).spacing(SECTION_SEPARATION_PX),
            self.config.theme.palette.dim,
            DIVIDER_THICKNESS_PX,
        )
    }

    fn section(&self, section: &Section) -> Element<'static, Message> {
        let title_padding = Padding::ZERO.left(COLUMN_GAP_PX / 2.0);
        let title = section.title().map(|title| {
            container(self.bold_title(title))
                .padding(title_padding)
                .into()
        });
        let caption = section.caption().map(|caption| {
            container(self.small_dim_text(caption.to_owned()))
                .max_width(self.config.tooltips.caption_width_px)
                .padding(title_padding)
                .into()
        });

        inset(
            Column::with_children(
                title
                    .into_iter()
                    .chain(caption)
                    .chain(once(self.body(section.body()))),
            )
            .spacing(ROW_SPACING_PX)
            .into(),
        )
    }

    fn body(&self, body: &SectionBody) -> Element<'static, Message> {
        match body {
            SectionBody::Dictionary(entries) => self.dictionary(entries),
            SectionBody::ColumnTable { headings, records } => self.column_table(headings, records),
            SectionBody::Tree { root, folding } => {
                ProcessTree::new(root, folding, self.config, self.phase).view()
            }
            SectionBody::Estimated(sections) => self
                .in_phase(LoadingPhase::Estimated(self.config.tooltips.loading))
                .sections(sections),
            SectionBody::Resolved(sections) => self
                .in_phase(LoadingPhase::Arriving(self.config.tooltips.loading))
                .sections(sections),
        }
    }

    fn cell_view(&self) -> CellView {
        CellView {
            font: self.config.theme.font.clone(),
            max_width: self.config.tooltips.value_width_px,
            locale: self.config.calendar.locale,
            flash: self.flash(),
            link: self.config.theme.palette.info,
            hint: self.dim(),
            phase: self.phase,
            copy_flash: self.config.tooltips.copy_flash,
        }
    }

    fn dictionary(&self, entries: &[KeyValue]) -> Element<'static, Message> {
        let palette = self.config.theme.palette.clone();
        let cell_view = self.cell_view();
        let value_column = table::column(Space::new(), move |entry: KeyValue| {
            cell_view.value(entry.value().clone(), entry.severity().color(&palette))
        })
        .align_y(Vertical::Top);

        self.keyed_table(entries, KeyValue::key, once(value_column))
    }

    fn column_table(
        &self,
        headings: &[String],
        records: &[RecordCells],
    ) -> Element<'static, Message> {
        let heading_color = self.config.theme.palette.inactive;

        self.keyed_table(
            records,
            RecordCells::key,
            headings.iter().enumerate().map(|(position, heading)| {
                let heading =
                    column_heading(&self.config.theme.font, heading.clone(), heading_color);
                self.record_column(position, heading.into())
                    .align_x(Horizontal::Right)
            }),
        )
    }

    fn keyed_table<Entry: Clone + 'static>(
        &self,
        entries: &[Entry],
        key: fn(&Entry) -> &Cell,
        values: impl IntoIterator<Item = table::Column<'static, 'static, Entry, Message>>,
    ) -> Element<'static, Message> {
        let dim = self.dim();
        let label_view = CellView {
            phase: None,
            ..self.cell_view()
        };
        let key_column = table::column(Space::new(), move |entry: Entry| {
            label_view.key(key(&entry).clone(), dim)
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top);

        table::table(once(key_column).chain(values), entries.iter().cloned())
            .separator(0.0)
            .padding_x(COLUMN_GAP_PX / 2.0)
            .padding_y(ROW_SPACING_PX / 2.0)
            .into()
    }

    fn record_column(
        &self,
        position: usize,
        heading: Element<'static, Message>,
    ) -> table::Column<'static, 'static, RecordCells, Message> {
        let palette = self.config.theme.palette.clone();
        let cell_view = self.cell_view();

        table::column(heading, move |record: RecordCells| {
            match record.cell(position) {
                Some(value) => cell_view.value(value.clone(), record.severity().color(&palette)),
                None => Space::new().into(),
            }
        })
        .align_y(Vertical::Top)
    }
}

struct CellView {
    font: ResolvedFont,
    max_width: f32,
    locale: Locale,
    flash: Color,
    link: Color,
    hint: Color,
    phase: Option<LoadingPhase>,
    copy_flash: Duration,
}

impl CellView {
    fn key(&self, key: Cell, color: Color) -> Element<'static, Message> {
        match key {
            Cell::Glyph(icon) => self.glyph(icon, color),
            Cell::Text(text) | Cell::Link(text) => self.copyable_text(text, color),
            Cell::Hint(text) => self.text(text, self.hint),
            Cell::Time { at, part } => self.copyable_text(part.format(&at, self.locale), color),
        }
    }

    fn text(&self, text: String, color: Color) -> Element<'static, Message> {
        loading_text(
            cell(&self.font, text.clone(), color, self.max_width),
            text,
            self.phase,
            &self.font,
            color,
        )
    }

    fn value(&self, value: Cell, color: Color) -> Element<'static, Message> {
        match value {
            Cell::Glyph(icon) => self.glyph(icon, color),
            Cell::Text(text) => self.copyable_text(text, color),
            Cell::Time { at, part } => self.copyable_text(part.format(&at, self.locale), color),
            Cell::Link(url) => self.openable_link(url),
            Cell::Hint(text) => self.text(text, self.hint),
        }
    }

    fn openable_link(&self, url: String) -> Element<'static, Message> {
        let value = self.text(url.clone(), self.link);
        openable(value, url, Message::OpenUrl, self.link, self.copy_flash)
    }

    fn glyph(&self, icon: Glyph, color: Color) -> Element<'static, Message> {
        icon.view(self.font.text_size_px, color)
    }

    fn copyable_text(&self, text: String, color: Color) -> Element<'static, Message> {
        let value = self.text(text.clone(), color);
        if text.trim().is_empty() {
            return value;
        }
        copyable(value, text, Message::CopyText, self.flash, self.copy_flash)
    }
}

fn cell(
    font: &ResolvedFont,
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
