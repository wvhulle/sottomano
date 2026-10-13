use std::{
    iter::once,
    path::Path,
    time::{Duration, SystemTime},
};

use chrono::Locale;
use iced::{
    Color, Element, Length, Padding,
    alignment::{Horizontal, Vertical},
    border::Radius,
    mouse::Interaction,
    widget::{
        Column, Container, Space, Text, container, row,
        rule::{self, FillMode},
        table,
        text::Wrapping,
    },
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
        favicon::Favicons,
        phosphor::{Phosphor, Weight},
        pressable_text::pressable_text,
        refresh_dial::RefreshDial,
        text::{bar_text, column_heading},
        tooltip_body::{
            Cell, Freshness, KeyValue, Readiness, RecordCells, Section, SectionBody, SourcedBody,
            SourcedSection, Tooltip, TrayMenuBody, WholeBody,
        },
        transitioning_text::{TextPresence, TransitionStyle, TransitioningText},
        uniform_width_column::uniform_width_column,
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
const REFRESH_TRACK_ALPHA: f32 = 0.2;
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
    presence: TextPresence,
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
            presence: TextPresence::Shown,
        }
    }

    fn in_readiness(self, readiness: Readiness) -> Self {
        let presence = match readiness {
            Readiness::Settled => TextPresence::Shown,
            Readiness::Estimated => TextPresence::Placeholder,
        };
        Self { presence, ..self }
    }

    fn transition(&self, presence: TextPresence) -> TransitionStyle {
        TransitionStyle {
            loading: self.config.tooltips.loading,
            presence,
            highlight: self.flash(),
        }
    }

    pub fn view(self) -> Element<'static, Message> {
        let gestures = self.tooltip.press_hints.gestures();
        let press_hint = (!gestures.is_empty()).then(|| self.gesture_hints(&gestures));
        container(
            uniform_width_column(press_hint.into_iter().chain([self.grid()]))
                .spacing(FOOTER_GAP_PX),
        )
        .padding(SHELL_PADDING)
        .into()
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

    fn freshness(&self, freshness: Freshness) -> Element<'static, Message> {
        let dim = self.dim();
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>, glyph: Element<'static, Message>| {
            row([label.into(), glyph])
                .spacing(GESTURE_ICON_GAP_PX)
                .align_y(Vertical::Center)
                .into()
        };
        let shown = match freshness {
            Freshness::Polled(info) => pressable_text(
                labelled(
                    self.small_dim_text(humantime::format_duration(info.every).to_string()),
                    RefreshDial::new(
                        info.elapsed_since_last_poll(self.now),
                        glyph_size,
                        dim,
                        dim.scale_alpha(REFRESH_TRACK_ALPHA),
                    )
                    .view(),
                ),
                info.source,
                Message::RefreshSource,
                self.flash(),
                self.config.tooltips.copy_flash,
                Interaction::Pointer,
            ),
            Freshness::Live => labelled(
                self.annotation("live".to_owned()),
                Phosphor::Pulse(Weight::Regular).view(glyph_size, dim),
            ),
        };
        container(shown).align_right(Length::Fill).into()
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
            SourcedBody::Sections(sections) => self.sections(sections),
            SourcedBody::Whole { body, freshness } => {
                Column::with_children([self.whole(body), inset(self.freshness(*freshness))])
                    .spacing(ROW_SPACING_PX)
                    .into()
            }
        }
    }

    fn whole(&self, body: &WholeBody) -> Element<'static, Message> {
        match body {
            WholeBody::Workspace(preview) => inset(
                WorkspaceSchematic::new(preview, self.config, self.corner_radius, self.favicons)
                    .view(),
            ),
            WholeBody::Text { title, body } => inset(self.text(title, body.as_deref())),
            WholeBody::TrayMenu(menu) => self.tray_menu(menu),
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

    fn sections(&self, sections: &[SourcedSection]) -> Element<'static, Message> {
        let filled = sections
            .iter()
            .filter(|section| !section.section().body().is_empty())
            .map(|sourced| {
                self.in_readiness(sourced.readiness())
                    .section(sourced.section(), self.freshness(sourced.freshness()))
            });

        let dim = self.config.theme.palette.dim;
        let divider = move || {
            rule::horizontal(DIVIDER_THICKNESS_PX)
                .style(move |_| rule::Style {
                    color: dim,
                    radius: 0.0.into(),
                    fill_mode: FillMode::Full,
                    snap: false,
                })
                .into()
        };
        let with_dividers = filled.enumerate().flat_map(|(index, section)| {
            (index > 0).then(divider).into_iter().chain(once(section))
        });
        uniform_width_column(with_dividers)
            .spacing((SECTION_SEPARATION_PX - DIVIDER_THICKNESS_PX) / 2.0)
            .into()
    }

    fn section(
        &self,
        section: &Section,
        freshness: Element<'static, Message>,
    ) -> Element<'static, Message> {
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
                    .chain([self.body(section.body()), freshness]),
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
                ProcessTree::new(root, folding, self.config, self.transition(self.presence)).view()
            }
        }
    }

    fn cell_view(&self) -> CellView {
        CellView {
            font: self.config.theme.monospace_font.clone(),
            max_width: self.config.tooltips.value_width_px,
            locale: self.config.calendar.locale,
            flash: self.flash(),
            link: self.config.theme.palette.info,
            hint: self.dim(),
            transition: self.transition(self.presence),
            copy_flash: self.config.tooltips.copy_flash,
        }
    }

    fn dictionary(&self, entries: &[KeyValue]) -> Element<'static, Message> {
        let palette = self.config.theme.palette.clone();
        let cell_view = self.cell_view();
        let value_column = table::column(Space::new(), move |entry: KeyValue| {
            cell_view.value(
                entry.value().clone(),
                entry.severity().color(&palette),
                entry.key(),
            )
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
            font: self.config.theme.font.clone(),
            transition: self.transition(TextPresence::Shown),
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
                Some(value) => cell_view.value(
                    value.clone(),
                    record.severity().color(&palette),
                    record.key(),
                ),
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
    transition: TransitionStyle,
    copy_flash: Duration,
}

impl CellView {
    fn key(&self, key: Cell, color: Color) -> Element<'static, Message> {
        let row = key.clone();
        match key {
            Cell::Glyph(icon) => self.glyph(icon, color),
            Cell::Text(text) | Cell::Link(text) => self.copyable_text(text, color, &row),
            Cell::Path(path) => self.path(&path, color, &row),
            Cell::Hint(text) => self.text(text, self.hint, &row),
            Cell::Time { at, part } => {
                self.copyable_text(part.format(&at, self.locale), color, &row)
            }
        }
    }

    fn text(&self, text: String, color: Color, row: &Cell) -> Element<'static, Message> {
        TransitioningText::new(
            cell(&self.font, text.clone(), color, self.max_width),
            text,
            self.transition,
            &self.font,
            color,
        )
        .identified_by(row)
        .into()
    }

    fn path(&self, path: &Path, color: Color, row: &Cell) -> Element<'static, Message> {
        let text = path.display().to_string();
        let unwrapped = bar_text(&self.font, text.clone())
            .color(color)
            .wrapping(Wrapping::None);
        let value =
            TransitioningText::new(unwrapped, text.clone(), self.transition, &self.font, color)
                .identified_by(row);
        pressable_text(
            value,
            text,
            Message::CopyText,
            self.flash,
            self.copy_flash,
            Interaction::Copy,
        )
    }

    fn value(&self, value: Cell, color: Color, row: &Cell) -> Element<'static, Message> {
        match value {
            Cell::Glyph(icon) => self.glyph(icon, color),
            Cell::Text(text) => self.copyable_text(text, color, row),
            Cell::Path(path) => self.path(&path, color, row),
            Cell::Time { at, part } => {
                self.copyable_text(part.format(&at, self.locale), color, row)
            }
            Cell::Link(url) => self.openable_link(url, row),
            Cell::Hint(text) => self.text(text, self.hint, row),
        }
    }

    fn openable_link(&self, url: String, row: &Cell) -> Element<'static, Message> {
        let value = self.text(url.clone(), self.link, row);
        pressable_text(
            value,
            url,
            Message::OpenUrl,
            self.link,
            self.copy_flash,
            Interaction::Pointer,
        )
    }

    fn glyph(&self, icon: Glyph, color: Color) -> Element<'static, Message> {
        icon.view(self.font.text_size_px, color)
    }

    fn copyable_text(&self, text: String, color: Color, row: &Cell) -> Element<'static, Message> {
        let value = self.text(text.clone(), color, row);
        if text.trim().is_empty() {
            return value;
        }
        pressable_text(
            value,
            text,
            Message::CopyText,
            self.flash,
            self.copy_flash,
            Interaction::Copy,
        )
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

fn whole_seconds_between(at: SystemTime, now: SystemTime) -> humantime::FormattedDuration {
    let whole_seconds = now.duration_since(at).map_or(0, |age| age.as_secs());
    humantime::format_duration(Duration::from_secs(whole_seconds))
}

pub fn relative(at: SystemTime, now: SystemTime) -> String {
    format!("{} ago", whole_seconds_between(at, now))
}

pub fn ongoing_for(since: SystemTime, now: SystemTime) -> String {
    format!("for {}", whole_seconds_between(since, now))
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
