use std::{
    iter::once,
    path::Path,
    time::{Duration, SystemTime},
};

use chrono::Locale;
use humantime::format_duration;
use iced::{
    Color, Element, Length, Padding,
    alignment::{Horizontal, Vertical},
    mouse::Interaction,
    widget::{
        Column, Container, Space, Text, container, rich_text, row,
        rule::{self, FillMode},
        span, table,
        text::{Span, Wrapping},
    },
};
use iced_controller_diagram::{Style as DiagramStyle, controller_diagram};
use sottomano_config::{Config, theme::font::ResolvedFont};
use sottomano_state::idle::ScheduledAfterIdle;

use crate::{
    clock::Clock,
    hover_tooltips::{MouseGesture, PressHints, core_grid::CoreCellGrid},
    shell_message::Message,
    widget_groups::{
        session_menu::SessionMessage,
        tray_group::{TrayMessage, menu_lines::TrayMenuLines},
        window_group::process_tree::ProcessTree,
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        bar_widget::Glyph,
        context_menu::{MenuLine, MenuRow, Trailing, context_menu},
        palette_colors::PaletteColors,
        phosphor::{Phosphor, Weight},
        pressable_text::pressable_text,
        refresh_dial::RefreshDial,
        text::{FontFaces, LINE_HEIGHT, bar_text, column_heading},
        tooltip_body::{
            Cell, ChipletCells, ColumnHeading, ControllerCallouts, Freshness, KeyValue, Readiness,
            RecordCells, Section, SectionBody, SessionMenuBody, SessionMenuEntry, Tooltip,
            TooltipBody, TrayMenuBody, WholeBody,
        },
        transitioning_text::{TextPresence, TransitionStyle, TransitioningText},
        uniform_width_column::uniform_width_column,
        widget_state::WidgetState,
        window_icon::WindowAssets,
    },
};

const COLUMN_GAP_PX: f32 = 14.0;
const FOOTER_GAP_PX: f32 = 6.0;
const SECTION_GAP_PX: f32 = 8.0;
const TRAY_MENU_WIDTH_PX: f32 = 250.0;
const SECTION_SEPARATION_PX: f32 = 18.0;
const DIVIDER_THICKNESS_PX: f32 = 1.0;
const REFRESH_TRACK_ALPHA: f32 = 0.2;
const DIM_ALPHA: f32 = 0.7;
const CONTROLLER_DIAGRAM_WIDTH_PX: f32 = 300.0;
const GESTURE_ICON_GAP_PX: f32 = 4.0;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
};

#[derive(Clone, Copy)]
pub struct TooltipContent<'a> {
    tooltip: &'a Tooltip,
    config: &'a Config,
    assets: WindowAssets<'a>,
    clock: Clock,
    presence: TextPresence,
}

impl<'a> TooltipContent<'a> {
    #[must_use]
    pub fn new(
        tooltip: &'a Tooltip,
        config: &'a Config,
        assets: WindowAssets<'a>,
        clock: Clock,
    ) -> Self {
        Self {
            tooltip,
            config,
            assets,
            clock,
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

    #[must_use]
    pub fn view(self) -> Element<'static, Message> {
        let press_hints = self.tooltip.press_hints;
        let press_hint = (!press_hints.is_empty()).then(|| self.gesture_hints(press_hints));
        container(
            uniform_width_column(press_hint.into_iter().chain([self.grid()]))
                .spacing(FOOTER_GAP_PX),
        )
        .padding(self.shell_padding())
        .into()
    }

    fn content_inset(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding::ZERO.left(inset).right(inset)
    }

    fn shell_padding(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding::ZERO.top(inset).bottom(inset)
    }

    fn footer_padding(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding {
            left: FOOTER_PADDING.left + inset,
            right: FOOTER_PADDING.right + inset,
            ..FOOTER_PADDING
        }
    }

    fn inset(&self, content: Element<'static, Message>) -> Element<'static, Message> {
        container(content).padding(self.content_inset()).into()
    }

    fn gesture_hints(&self, press_hints: PressHints) -> Element<'static, Message> {
        let icon_size = self.config.theme.font.small_text_size_px;
        let dim = self.dim();
        container(
            row(MouseGesture::ALL.into_iter().filter_map(|gesture| {
                let does = press_hints.does(gesture)?;
                Some(
                    row([
                        Glyph::from(gesture.glyph()).view(icon_size, dim),
                        self.annotation(does.to_owned()).into(),
                    ])
                    .spacing(GESTURE_ICON_GAP_PX)
                    .align_y(Vertical::Center)
                    .into(),
                )
            }))
            .spacing(COLUMN_GAP_PX),
        )
        .padding(self.footer_padding())
        .into()
    }

    fn foreground(&self) -> Color {
        PaletteColors::from(&self.config.theme.palette).bar_foreground
    }

    fn flash(&self) -> Color {
        PaletteColors::from(&self.config.theme.palette).success
    }

    fn dim(&self) -> Color {
        Color {
            a: DIM_ALPHA,
            ..PaletteColors::from(&self.config.theme.palette).bar_foreground
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
            Freshness::Polled { every, last } => pressable_text(
                labelled(
                    self.small_dim_text(humantime::format_duration(every).to_string()),
                    RefreshDial::new(
                        self.clock
                            .wall
                            .duration_since(last)
                            .unwrap_or(Duration::ZERO),
                        every,
                        self.clock.instant,
                        glyph_size,
                        dim,
                        dim.scale_alpha(REFRESH_TRACK_ALPHA),
                    )
                    .view(),
                ),
                (),
                |()| Message::RefreshHoveredReading,
                self.flash(),
                self.config.tooltips.copy_flash,
                Interaction::Pointer,
            ),
            Freshness::Live => labelled(
                self.annotation("live".to_owned()),
                Phosphor::Pulse(Weight::Regular).view(glyph_size, dim),
            ),
            Freshness::Fixed => labelled(
                self.annotation("read once".to_owned()),
                Phosphor::PushPin(Weight::Regular).view(glyph_size, dim),
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
            .font(FontFaces::from(&self.config.theme.font).italic)
    }

    fn grid(&self) -> Element<'static, Message> {
        match &self.tooltip.body {
            TooltipBody::Sections(sections) => self.sections(sections),
            TooltipBody::Whole { body, freshness } => {
                Column::with_children([self.whole(body), self.inset(self.freshness(*freshness))])
                    .spacing(self.config.layout.overlay.row_spacing)
                    .into()
            }
        }
    }

    fn whole(&self, body: &WholeBody) -> Element<'static, Message> {
        match body {
            WholeBody::Workspace(preview) => {
                self.inset(WorkspaceSchematic::new(preview, self.config, self.assets).view())
            }
            WholeBody::Text { title, body } => self.inset(self.text(title, body.as_deref())),
            WholeBody::TrayMenu(menu) => self.tray_menu(menu),
            WholeBody::SessionMenu(menu) => self.session_menu(menu),
        }
    }

    fn tray_menu(&self, menu: &TrayMenuBody) -> Element<'static, Message> {
        let address = menu.address.clone();
        let menu_path = menu.menu_path.clone();
        let on_activate = move |id| {
            Message::Tray(TrayMessage::MenuItemClicked {
                address: address.clone(),
                menu_path: menu_path.clone(),
                id,
            })
        };
        let on_toggle = |id| Message::Tray(TrayMessage::ToggleSubmenu(id));
        let lines = TrayMenuLines {
            config: self.config,
            pixmap_handles: &menu.pixmap_handles,
            open: &menu.open_submenus,
            on_activate: &on_activate,
            on_toggle: &on_toggle,
        }
        .lines(&menu.nodes);
        let entries = context_menu(self.config, lines);
        Column::new()
            .spacing(SECTION_GAP_PX)
            .push(
                container(self.text(&menu.title, menu.body.as_deref()))
                    .padding(self.content_inset())
                    .width(TRAY_MENU_WIDTH_PX),
            )
            .push(container(entries).width(TRAY_MENU_WIDTH_PX))
            .into()
    }

    fn session_menu(&self, menu: &SessionMenuBody) -> Element<'static, Message> {
        let lines = menu
            .entries
            .iter()
            .scan(None, |previous_family, entry| {
                let family = entry.action.family();
                let divider = previous_family.is_some_and(|previous| previous != family);
                *previous_family = Some(family);
                Some(
                    divider
                        .then_some(MenuLine::Divider)
                        .into_iter()
                        .chain(once(self.session_menu_row(*entry))),
                )
            })
            .flatten();
        container(context_menu(self.config, lines))
            .width(TRAY_MENU_WIDTH_PX)
            .into()
    }

    fn session_menu_row(&self, entry: SessionMenuEntry) -> MenuLine<'static, Message> {
        let icon_size = self.config.theme.font.icon_size_px;
        let note = match entry.idle {
            ScheduledAfterIdle::After(after) => {
                Trailing::Note(format!("after {} idle", format_duration(after)))
            }
            ScheduledAfterIdle::Disabled => Trailing::Note("idle: disabled".to_owned()),
            ScheduledAfterIdle::Never if entry.needs_authorisation => {
                Trailing::Note("asks for authorisation".to_owned())
            }
            ScheduledAfterIdle::Never => Trailing::Nothing,
        };
        MenuLine::Row(MenuRow {
            leading: Some(
                Glyph::from(Phosphor::from(entry.action)).view(icon_size, self.foreground()),
            ),
            label: Some(entry.action.verb().to_owned()),
            trailing: note,
            enabled: true,
            depth: 0,
            on_press: Some(Message::Session(SessionMessage::Press(entry.action))),
        })
    }

    fn bold_title(&self, title: &str) -> Text<'static> {
        bar_text(&self.config.theme.font, title.to_owned())
            .font(FontFaces::from(&self.config.theme.font).bold)
            .color(self.foreground())
    }

    fn text(&self, title: &str, body: Option<&str>) -> Element<'static, Message> {
        let max_width = self.config.tooltips.value_width;
        let title = container(self.bold_title(title)).max_width(max_width);
        let body = body.map(|body| {
            cell(
                &self.config.theme.font,
                body.to_owned(),
                self.dim(),
                max_width,
                Wrapping::default(),
            )
        });

        Column::with_children(once(title).chain(body).map(Element::from))
            .spacing(self.config.layout.overlay.row_spacing)
            .into()
    }

    fn sections(&self, sections: &[Section]) -> Element<'static, Message> {
        let filled = sections
            .iter()
            .filter(|section| !section.body().is_empty())
            .map(|section| {
                self.in_readiness(section.readiness())
                    .section(section, self.freshness(section.freshness()))
            });

        let dim = PaletteColors::from(&self.config.theme.palette).dim;
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
                .max_width(self.config.tooltips.value_width)
                .padding(title_padding)
                .into()
        });

        self.inset(
            Column::with_children(
                title
                    .into_iter()
                    .chain(caption)
                    .chain([self.body(section.body()), freshness]),
            )
            .spacing(self.config.layout.overlay.row_spacing)
            .into(),
        )
    }

    fn body(&self, body: &SectionBody) -> Element<'static, Message> {
        match body {
            SectionBody::Dictionary(entries) => self.dictionary(entries),
            SectionBody::ColumnTable { headings, records } => self.column_table(headings, records),
            SectionBody::CoreGrid(chiplets) => self.core_grid(chiplets),
            SectionBody::ControllerDiagram(diagram) => self.controller_diagram(diagram),
            SectionBody::Tree { root, folding } => {
                ProcessTree::new(root, folding, self.config, self.transition(self.presence)).view()
            }
        }
    }

    fn cell_view(&self) -> CellView {
        CellView {
            font: self.config.theme.monospace_font.clone(),
            max_width: self.config.tooltips.value_width,
            wrapping: Wrapping::default(),
            locale: self.config.calendar.locale,
            flash: self.flash(),
            link: PaletteColors::from(&self.config.theme.palette).info,
            hint: self.dim(),
            transition: self.transition(self.presence),
            copy_flash: self.config.tooltips.copy_flash,
        }
    }

    fn dictionary(&self, entries: &[KeyValue]) -> Element<'static, Message> {
        let palette = PaletteColors::from(&self.config.theme.palette);
        let cell_view = self.cell_view();
        let value_column = table::column(Space::new(), move |entry: KeyValue| {
            cell_view.value(
                entry.value().clone(),
                WidgetState::from(entry.severity()).color(&palette),
                entry.key(),
            )
        })
        .align_y(Vertical::Top);

        self.keyed_table(entries, KeyValue::key, once(value_column))
    }

    fn controller_diagram(&self, diagram: &ControllerCallouts) -> Element<'static, Message> {
        let palette = PaletteColors::from(&self.config.theme.palette);
        let font = &self.config.theme.font;
        let leader_style = DiagramStyle {
            line: self.dim(),
            anchor: palette.info,
        };
        let row_height = LINE_HEIGHT.to_absolute(font.text_size_px.into()).0.ceil();
        diagram
            .callouts
            .iter()
            .fold(
                controller_diagram(self.assets.controller_artwork.clone(), diagram.model)
                    .body_width(CONTROLLER_DIAGRAM_WIDTH_PX)
                    .row_height(row_height)
                    .style(move |_| leader_style),
                |drawn, callout| {
                    drawn.callout(
                        callout.anchor,
                        bar_text(font, callout.label.clone())
                            .color(WidgetState::from(callout.level).color(&palette)),
                    )
                },
            )
            .into()
    }

    fn core_grid(&self, chiplets: &[ChipletCells]) -> Element<'static, Message> {
        let spacing = self.config.layout.overlay.row_spacing;
        Column::with_children(chiplets.iter().map(|chiplet| {
            let cells = container(CoreCellGrid::new(&chiplet.cores, self.config).view())
                .padding(Padding::ZERO.left(COLUMN_GAP_PX / 2.0));
            Column::with_children([
                container(
                    bar_text(&self.config.theme.font, chiplet.title.clone())
                        .color(self.foreground()),
                )
                .padding(Padding::ZERO.left(COLUMN_GAP_PX / 2.0))
                .into(),
                row![cells, self.dictionary(&chiplet.facts)]
                    .spacing(COLUMN_GAP_PX)
                    .align_y(Vertical::Center)
                    .into(),
            ])
            .spacing(spacing / 2.0)
            .into()
        }))
        .spacing(spacing)
        .into()
    }

    fn column_table(
        &self,
        headings: &[ColumnHeading],
        records: &[RecordCells],
    ) -> Element<'static, Message> {
        let heading_color = PaletteColors::from(&self.config.theme.palette).inactive;

        self.keyed_table(
            records,
            RecordCells::key,
            headings.iter().enumerate().map(|(position, heading)| {
                let label = column_heading(&self.config.theme.font, heading.label(), heading_color);
                self.record_column(position, label.into())
                    .align_x(heading.alignment())
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
            wrapping: Wrapping::None,
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
            .padding_y(self.config.layout.overlay.row_spacing / 2.0)
            .into()
    }

    fn record_column(
        &self,
        position: usize,
        heading: Element<'static, Message>,
    ) -> table::Column<'static, 'static, RecordCells, Message> {
        let palette = PaletteColors::from(&self.config.theme.palette);
        let cell_view = self.cell_view();

        table::column(heading, move |record: RecordCells| {
            match record.cell(position) {
                Some(value) => cell_view.value(
                    value.clone(),
                    WidgetState::from(record.severity()).color(&palette),
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
    wrapping: Wrapping,
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
            Cell::Text(text) => self.copyable_text(text, color, &row),
            Cell::Link { label, url } => self.openable_link(label, url, color, &row),
            Cell::Path(path) => self.path(&path, color, &row),
            Cell::Hint(text) => self.text(text, self.hint, &row),
            Cell::Time { at, format } => {
                self.copyable_text(format.format(&at, self.locale), color, &row)
            }
        }
    }

    fn text(&self, text: String, color: Color, row: &Cell) -> Element<'static, Message> {
        TransitioningText::new(
            cell(
                &self.font,
                text.clone(),
                color,
                self.max_width,
                self.wrapping,
            ),
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
            Cell::Time { at, format } => {
                self.copyable_text(format.format(&at, self.locale), color, row)
            }
            Cell::Link { label, url } => self.openable_link(label, url, color, row),
            Cell::Hint(text) => self.text(text, self.hint, row),
        }
    }

    fn openable_link(
        &self,
        label: String,
        url: String,
        color: Color,
        row: &Cell,
    ) -> Element<'static, Message> {
        let underlined_label: Span<'static> = span(label.clone())
            .font(FontFaces::from(&self.font).regular)
            .color(color)
            .underline(true);
        let underlined = container(
            rich_text([underlined_label])
                .size(self.font.text_size_px)
                .line_height(LINE_HEIGHT)
                .wrapping(self.wrapping),
        )
        .max_width(self.max_width);
        let value = TransitioningText::new(underlined, label, self.transition, &self.font, color)
            .identified_by(row);
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
    wrapping: Wrapping,
) -> Container<'static, Message> {
    container(bar_text(font, text).color(color).wrapping(wrapping)).max_width(max_width)
}

fn whole_seconds_between(at: SystemTime, now: SystemTime) -> humantime::FormattedDuration {
    let whole_seconds = now.duration_since(at).map_or(0, |age| age.as_secs());
    humantime::format_duration(Duration::from_secs(whole_seconds))
}

#[must_use]
pub fn relative(at: SystemTime, now: SystemTime) -> String {
    format!("{} ago", whole_seconds_between(at, now))
}

#[must_use]
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
