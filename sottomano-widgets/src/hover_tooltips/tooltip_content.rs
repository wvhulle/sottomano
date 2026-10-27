use std::{
    iter::once,
    num::NonZeroUsize,
    path::Path,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Locale, Utc};
use iced::{
    Color, Element, Length, Padding, Pixels,
    alignment::{Horizontal, Vertical},
    mouse::Interaction,
    widget::{
        Column, Container, Space, Text, container, rich_text, row, span,
        text::{Span, Wrapping},
    },
};
use iced_controller_diagram::{Style as DiagramStyle, controller_diagram};
use sottomano_config::{Config, theme::font::ResolvedFont};

use crate::{
    clock::Clock,
    hover_tooltips::{MouseGesture, PopupMessage, PressHints, core_grid::CoreCellGrid},
    widget_groups::{
        window_group::process_tree::ProcessTree,
        workspaces_group::workspace_preview::WorkspaceSchematic,
    },
    widgets::{
        activity_line::activity_line,
        bar_widget::Glyph,
        content_sized_table::{ContentSizedColumn, content_sized_column, content_sized_table},
        ellipsis_rule::ellipsis_rule,
        localized_durations::{duration_text, relative_moment_text, signed_duration_text},
        palette_colors::PaletteColors,
        phosphor::{Phosphor, Weight},
        pressable_text::pressable_text,
        refresh_dial::RefreshDial,
        text::{FontFaces, LINE_HEIGHT, bar_text, column_heading},
        tooltip_body::{
            Cell, ChipletCells, ColumnHeading, ControllerCallouts, DeliveryTiming, KeyValue,
            RecordCells, Section, SectionBody, SectionsDeliveryState, SourcedSections,
            TableColumns, TableRows, Tooltip,
        },
        transitioning_text::{TextPresence, TransitionStyle, TransitioningText},
        uniform_width_column::uniform_width_column,
        widget_state::WidgetState,
        window_icon::WindowAssets,
    },
};

const COLUMN_GAP_PX: f32 = 14.0;
const SECTION_SEPARATION_PX: f32 = 16.0;
const REFRESH_TRACK_ALPHA: f32 = 0.2;
const CONTROLLER_DIAGRAM_WIDTH_PX: f32 = 300.0;
const GESTURE_ICON_GAP_PX: f32 = 4.0;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 0.0,
    bottom: 2.0,
    left: COLUMN_GAP_PX / 2.0,
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

    fn in_delivery_state(self, delivery_state: SectionsDeliveryState) -> Self {
        let presence = match delivery_state {
            SectionsDeliveryState::Delivered { .. } => TextPresence::Shown,
            SectionsDeliveryState::AwaitingDelivery => TextPresence::Placeholder,
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
    pub fn view(self) -> Element<'static, PopupMessage> {
        let groups: Vec<&SourcedSections> = self.tooltip.filled_groups().collect();
        let Some((closing_group, leading_groups)) = groups.split_last() else {
            return container(uniform_width_column([self.closing_row(None)]))
                .padding(self.shell_padding())
                .into();
        };
        let leading = leading_groups
            .iter()
            .map(|group| -> Element<'static, PopupMessage> {
                uniform_width_column([
                    self.group_sections(group),
                    self.footer_row(None, Some(self.delivery_state_view(group.delivery_state()))),
                ])
                .spacing(self.config.layout.overlay.row_spacing)
                .into()
            });
        let closing: Element<'static, PopupMessage> = uniform_width_column([
            self.group_sections(closing_group),
            self.closing_row(Some(closing_group.delivery_state())),
        ])
        .spacing(SECTION_SEPARATION_PX / 2.0)
        .into();
        container(uniform_width_column(leading.chain(once(closing))).spacing(SECTION_SEPARATION_PX))
            .padding(self.shell_padding())
            .into()
    }

    fn group_sections(&self, group: &SourcedSections) -> Element<'static, PopupMessage> {
        let content = self.in_delivery_state(group.delivery_state());
        let source_heading = group.source_heading().map(|heading| {
            self.inset(
                Column::with_children(self.heading_rows(Some(heading.title()), heading.caption()))
                    .spacing(self.config.layout.overlay.row_spacing)
                    .into(),
            )
        });
        let sections: Element<'static, PopupMessage> = uniform_width_column(
            group
                .filled_sections()
                .map(|section| content.section(section)),
        )
        .spacing(SECTION_SEPARATION_PX)
        .into();
        uniform_width_column(source_heading.into_iter().chain(once(sections)))
            .spacing(self.config.layout.overlay.row_spacing)
            .into()
    }

    fn closing_row(
        &self,
        closing_delivery_state: Option<SectionsDeliveryState>,
    ) -> Element<'static, PopupMessage> {
        let press_hints = self.tooltip.press_hints;
        let hints = (!press_hints.is_empty()).then(|| self.gesture_hints(press_hints));
        self.footer_row(
            hints,
            closing_delivery_state.map(|delivery_state| self.delivery_state_view(delivery_state)),
        )
    }

    fn delivery_state_view(
        &self,
        delivery_state: SectionsDeliveryState,
    ) -> Element<'static, PopupMessage> {
        match delivery_state {
            SectionsDeliveryState::Delivered { delivery_timing } => {
                self.delivery_timing(delivery_timing)
            }
            SectionsDeliveryState::AwaitingDelivery => row([
                self.small_dim_text("loading".to_owned()).into(),
                Glyph::from(Phosphor::HourglassMedium(Weight::Regular)).view(
                    self.config.theme.font.small_text_size_px,
                    self.secondary_foreground(),
                ),
            ])
            .spacing(GESTURE_ICON_GAP_PX)
            .align_y(Vertical::Center)
            .into(),
        }
    }

    fn footer_row(
        &self,
        hints: Option<Element<'static, PopupMessage>>,
        delivery_timing: Option<Element<'static, PopupMessage>>,
    ) -> Element<'static, PopupMessage> {
        container(
            row(hints
                .into_iter()
                .chain(once(Space::new().width(Length::Fill).into()))
                .chain(delivery_timing))
            .spacing(COLUMN_GAP_PX)
            .align_y(Vertical::Center),
        )
        .padding(self.footer_padding())
        .into()
    }

    fn content_inset(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding::ZERO.horizontal(inset)
    }

    fn shell_padding(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding::ZERO.vertical(inset)
    }

    fn footer_padding(&self) -> Padding {
        let inset = self.config.layout.overlay.content_inset;
        Padding {
            left: FOOTER_PADDING.left + inset,
            right: FOOTER_PADDING.right + inset,
            ..FOOTER_PADDING
        }
    }

    fn inset(&self, content: Element<'static, PopupMessage>) -> Element<'static, PopupMessage> {
        container(content).padding(self.content_inset()).into()
    }

    fn gesture_hints(&self, press_hints: PressHints) -> Element<'static, PopupMessage> {
        let icon_size = self.config.theme.font.small_text_size_px;
        let secondary_foreground = self.secondary_foreground();
        row(MouseGesture::ALL.into_iter().filter_map(|gesture| {
            let does = press_hints.does(gesture)?;
            Some(
                row([
                    Glyph::from(gesture.glyph()).view(icon_size, secondary_foreground),
                    self.small_dim_text(does.to_owned()).into(),
                ])
                .spacing(GESTURE_ICON_GAP_PX)
                .align_y(Vertical::Center)
                .into(),
            )
        }))
        .spacing(COLUMN_GAP_PX)
        .into()
    }

    fn foreground(&self) -> Color {
        PaletteColors::from(&self.config.theme.palette).bar_foreground
    }

    fn flash(&self) -> Color {
        PaletteColors::from(&self.config.theme.palette).success
    }

    fn secondary_foreground(&self) -> Color {
        PaletteColors::from(&self.config.theme.palette).secondary_foreground
    }

    fn delivery_timing(&self, delivery_timing: DeliveryTiming) -> Element<'static, PopupMessage> {
        let secondary_foreground = self.secondary_foreground();
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>, glyph: Element<'static, PopupMessage>| {
            row([label.into(), glyph])
                .spacing(GESTURE_ICON_GAP_PX)
                .align_y(Vertical::Center)
                .into()
        };
        match delivery_timing {
            DeliveryTiming::PeriodicPolling {
                poll_period,
                delivered_at,
            } => self.refreshable_dial(
                duration_text(poll_period, self.config.calendar.locale),
                poll_period,
                delivered_at,
            ),
            DeliveryTiming::LivePushWithRecheck {
                recheck_period,
                delivered_at,
            } => self.refreshable_dial(
                format!(
                    "live, recheck {}",
                    duration_text(recheck_period, self.config.calendar.locale)
                ),
                recheck_period,
                delivered_at,
            ),
            DeliveryTiming::LivePush => labelled(
                self.small_dim_text("live".to_owned()),
                Glyph::from(Phosphor::Pulse(Weight::Regular))
                    .view(glyph_size, secondary_foreground),
            ),
            DeliveryTiming::SingleRead => labelled(
                self.small_dim_text("read once".to_owned()),
                Glyph::from(Phosphor::PushPin(Weight::Regular))
                    .view(glyph_size, secondary_foreground),
            ),
        }
    }

    fn refreshable_dial(
        &self,
        label_text: String,
        refresh_period: Duration,
        delivered_at: SystemTime,
    ) -> Element<'static, PopupMessage> {
        let secondary_foreground = self.secondary_foreground();
        let glyph_size = self.config.theme.font.small_text_size_px;
        pressable_text(
            row([
                self.small_dim_text(label_text).into(),
                RefreshDial::new(
                    self.clock
                        .wall
                        .duration_since(delivered_at)
                        .unwrap_or(Duration::ZERO),
                    refresh_period,
                    self.clock.instant,
                    glyph_size,
                    secondary_foreground,
                    secondary_foreground.scale_alpha(REFRESH_TRACK_ALPHA),
                )
                .view(),
            ])
            .spacing(GESTURE_ICON_GAP_PX)
            .align_y(Vertical::Center),
            (),
            |()| PopupMessage::RefreshHoveredReading,
            self.flash(),
            self.config.tooltips.copy_flash,
            Interaction::Pointer,
        )
    }

    fn small_dim_text(&self, text: String) -> Text<'static> {
        bar_text(&self.config.theme.font, text)
            .size(self.config.theme.font.small_text_size_px)
            .color(self.secondary_foreground())
    }

    fn bold_title(&self, title: &str) -> Text<'static> {
        bar_text(&self.config.theme.font, title.to_owned())
            .font(FontFaces::from(&self.config.theme.font).bold)
            .color(self.foreground())
    }

    fn text(&self, title: &str, body: Option<&str>) -> Element<'static, PopupMessage> {
        let max_width = self.config.tooltips.value_width;
        let title = container(self.bold_title(title)).max_width(max_width);
        let body = body.map(|body| {
            cell(
                &self.config.theme.font,
                body.to_owned(),
                self.secondary_foreground(),
                max_width,
                Wrapping::default(),
            )
        });

        Column::with_children(once(title).chain(body).map(Element::from))
            .spacing(self.config.layout.overlay.row_spacing)
            .into()
    }

    fn section_seeding(&self) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let color = self.secondary_foreground();
        let line_height_px = LINE_HEIGHT.to_absolute(Pixels(font.text_size_px)).0;
        TransitioningText::new(
            container(Space::new())
                .width(self.config.tooltips.value_width)
                .height(line_height_px),
            String::new(),
            self.transition(TextPresence::Placeholder),
            font,
            color,
        )
        .into()
    }

    fn section(&self, section: &Section) -> Element<'static, PopupMessage> {
        self.inset(
            Column::with_children(
                self.heading_rows(section.title(), section.caption())
                    .chain(once(self.body(section.body()))),
            )
            .spacing(self.config.layout.overlay.row_spacing)
            .into(),
        )
    }

    fn heading_rows(
        &self,
        title: Option<&str>,
        caption: Option<&str>,
    ) -> impl Iterator<Item = Element<'static, PopupMessage>> {
        let title_padding = Padding::ZERO.left(COLUMN_GAP_PX / 2.0);
        let title = title.map(|title| {
            container(self.bold_title(title))
                .padding(title_padding)
                .into()
        });
        let caption = caption.map(|caption| {
            container(self.small_dim_text(caption.to_owned()))
                .max_width(self.config.tooltips.value_width)
                .padding(title_padding)
                .into()
        });
        title.into_iter().chain(caption)
    }

    fn body(&self, body: &SectionBody) -> Element<'static, PopupMessage> {
        match body {
            SectionBody::Dictionary(entries) => self.dictionary(entries),
            SectionBody::ColumnTable { columns, rows } => self.column_table(columns, rows),
            SectionBody::CoreGrid(chiplets) => self.core_grid(chiplets),
            SectionBody::ControllerDiagram(diagram) => self.controller_diagram(diagram),
            SectionBody::Tree { root, folding } => {
                ProcessTree::new(root, folding, self.config, self.transition(self.presence)).view()
            }
            SectionBody::Note { title, body } => self.text(title, body.as_deref()),
            SectionBody::SectionSeeding => self.section_seeding(),
            SectionBody::Workspace(preview) => {
                WorkspaceSchematic::new(preview, self.config, self.assets).view()
            }
        }
    }

    fn cell_view(&self) -> CellView {
        CellView {
            figure_font: self.config.theme.monospace_font.clone(),
            prose_font: self.config.theme.font.clone(),
            max_width: self.config.tooltips.value_width,
            wrapping: Wrapping::default(),
            locale: self.config.calendar.locale,
            now: self.clock.utc(),
            flash: self.flash(),
            link: PaletteColors::from(&self.config.theme.palette).info,
            hint: self.secondary_foreground(),
            transition: self.transition(self.presence),
            copy_flash: self.config.tooltips.copy_flash,
        }
    }

    fn dictionary(&self, entries: &[KeyValue]) -> Element<'static, PopupMessage> {
        let palette = PaletteColors::from(&self.config.theme.palette);
        let cell_view = self.cell_view();
        let value_column = content_sized_column(Space::new(), move |entry: KeyValue| {
            cell_view.value(
                entry.value().clone(),
                WidgetState::from(entry.severity()).color(&palette),
                entry.key(),
            )
        })
        .align_y(Vertical::Top);

        self.keyed_table(entries, KeyValue::key, once(value_column))
    }

    fn controller_diagram(&self, diagram: &ControllerCallouts) -> Element<'static, PopupMessage> {
        let palette = PaletteColors::from(&self.config.theme.palette);
        let font = &self.config.theme.font;
        let leader_style = DiagramStyle {
            line: self.secondary_foreground(),
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

    fn core_grid(&self, chiplets: &[ChipletCells]) -> Element<'static, PopupMessage> {
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
        columns: &TableColumns,
        rows: &TableRows,
    ) -> Element<'static, PopupMessage> {
        let headings = match columns {
            TableColumns::Headed(headings) => headings.clone(),
            TableColumns::Unheaded(alignments) => alignments
                .iter()
                .map(|alignment| ColumnHeading::untitled().align_x(*alignment))
                .collect(),
        };
        let key_foreground = self.foreground();
        let palette = PaletteColors::from(&self.config.theme.palette);
        let key_view = CellView {
            figure_font: self.config.theme.font.clone(),
            prose_font: self.config.theme.font.clone(),
            wrapping: Wrapping::None,
            transition: self.transition(TextPresence::Shown),
            ..self.cell_view()
        };
        let line_height_px = LINE_HEIGHT
            .to_absolute(Pixels(self.config.theme.font.text_size_px))
            .0;
        let key_column = content_sized_column(Space::new(), move |entry: TableEntry| match entry {
            TableEntry::Record(record) => key_view.key(record.key().clone(), key_foreground),
            TableEntry::OmittedRecords { omitted_count } => {
                key_view.key(Cell::from(format!("{omitted_count} more")), palette.dim)
            }
            TableEntry::ContinuationMarker => ellipsis_rule(palette.dim, line_height_px).into(),
            TableEntry::TotalRule => total_rule(palette.dim),
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top);
        let value_columns = headings.into_iter().enumerate().map(|(position, heading)| {
            self.record_column(position, self.column_heading_view(heading))
                .align_x(heading.alignment())
        });
        content_sized_table(once(key_column).chain(value_columns), table_entries(rows))
            .padding_x(COLUMN_GAP_PX / 2.0)
            .padding_y(self.config.layout.overlay.row_spacing / 2.0)
            .into()
    }

    fn column_heading_view(&self, heading: ColumnHeading) -> Element<'static, PopupMessage> {
        match heading {
            ColumnHeading::Titled { label, .. } => {
                column_heading(&self.config.theme.font, label, self.secondary_foreground()).into()
            }
            ColumnHeading::Untitled { .. } => Space::new().into(),
        }
    }

    fn keyed_table<Entry: Clone + 'static>(
        &self,
        entries: &[Entry],
        key: fn(&Entry) -> &Cell,
        values: impl IntoIterator<Item = ContentSizedColumn<'static, Entry, PopupMessage>>,
    ) -> Element<'static, PopupMessage> {
        let key_foreground = self.foreground();
        let label_view = CellView {
            figure_font: self.config.theme.font.clone(),
            prose_font: self.config.theme.font.clone(),
            wrapping: Wrapping::None,
            transition: self.transition(TextPresence::Shown),
            ..self.cell_view()
        };
        let key_column = content_sized_column(Space::new(), move |entry: Entry| {
            label_view.key(key(&entry).clone(), key_foreground)
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top);

        content_sized_table(once(key_column).chain(values), entries.iter().cloned())
            .padding_x(COLUMN_GAP_PX / 2.0)
            .padding_y(self.config.layout.overlay.row_spacing / 2.0)
            .into()
    }

    fn record_column(
        &self,
        position: usize,
        heading: Element<'static, PopupMessage>,
    ) -> ContentSizedColumn<'static, TableEntry, PopupMessage> {
        let palette = PaletteColors::from(&self.config.theme.palette);
        let cell_view = self.cell_view();
        let record_cell = move |record: &RecordCells, color: Color| match record.cell(position) {
            Some(value) => cell_view.value(value.clone(), color, record.key()),
            None => Space::new().into(),
        };
        let line_height_px = LINE_HEIGHT
            .to_absolute(Pixels(self.config.theme.font.text_size_px))
            .0;
        content_sized_column(heading, move |entry: TableEntry| match entry {
            TableEntry::Record(record) => {
                let severity_color = WidgetState::from(record.severity()).color(&palette);
                record_cell(&record, severity_color)
            }
            TableEntry::OmittedRecords { .. } | TableEntry::ContinuationMarker => {
                ellipsis_rule(palette.dim, line_height_px).into()
            }
            TableEntry::TotalRule => total_rule(palette.dim),
        })
        .align_y(Vertical::Top)
    }
}

#[derive(Clone)]
enum TableEntry {
    Record(RecordCells),
    OmittedRecords { omitted_count: NonZeroUsize },
    ContinuationMarker,
    TotalRule,
}

const TOTAL_RULE_THICKNESS_PX: f32 = 1.0;

fn total_rule(color: Color) -> Element<'static, PopupMessage> {
    activity_line(color, 1.0, TOTAL_RULE_THICKNESS_PX)
}

fn record_entries(records: &[RecordCells]) -> impl Iterator<Item = TableEntry> + '_ {
    records.iter().cloned().map(TableEntry::Record)
}

fn table_entries(rows: &TableRows) -> Vec<TableEntry> {
    match rows {
        TableRows::Complete(records) => record_entries(records).collect(),
        TableRows::Continuing(records) => record_entries(records)
            .chain(once(TableEntry::ContinuationMarker))
            .collect(),
        TableRows::Truncated {
            shown_records,
            omitted_count,
        } => record_entries(shown_records)
            .chain(once(TableEntry::OmittedRecords {
                omitted_count: *omitted_count,
            }))
            .collect(),
        TableRows::Totalled {
            shown_records,
            total_record,
        } => record_entries(shown_records)
            .chain([
                TableEntry::TotalRule,
                TableEntry::Record(total_record.clone()),
            ])
            .collect(),
    }
}

struct CellView {
    figure_font: ResolvedFont,
    prose_font: ResolvedFont,
    max_width: f32,
    wrapping: Wrapping,
    locale: Locale,
    now: DateTime<Utc>,
    flash: Color,
    link: Color,
    hint: Color,
    transition: TransitionStyle,
    copy_flash: Duration,
}

impl CellView {
    fn key(&self, key: Cell, color: Color) -> Element<'static, PopupMessage> {
        let row = key.clone();
        self.value(key, color, &row)
    }

    fn face(&self, face: TypeFace) -> &ResolvedFont {
        match face {
            TypeFace::Prose => &self.prose_font,
            TypeFace::Figures => &self.figure_font,
        }
    }

    fn text(
        &self,
        text: String,
        face: TypeFace,
        color: Color,
        row: &Cell,
    ) -> Element<'static, PopupMessage> {
        let font = self.face(face);
        TransitioningText::new(
            cell(font, text.clone(), color, self.max_width, self.wrapping),
            text,
            self.transition,
            font,
            color,
        )
        .identified_by(row)
        .into()
    }

    fn path(&self, path: &Path, color: Color, row: &Cell) -> Element<'static, PopupMessage> {
        let text = path.display().to_string();
        let unwrapped = bar_text(&self.prose_font, text.clone())
            .color(color)
            .wrapping(Wrapping::None);
        let value = TransitioningText::new(
            unwrapped,
            text.clone(),
            self.transition,
            &self.prose_font,
            color,
        )
        .identified_by(row);
        pressable_text(
            value,
            text,
            PopupMessage::CopyText,
            self.flash,
            self.copy_flash,
            Interaction::Copy,
        )
    }

    fn value(&self, value: Cell, color: Color, row: &Cell) -> Element<'static, PopupMessage> {
        match value {
            Cell::Glyph(icon) => self.glyph(icon, color),
            Cell::Text(text) => self.copyable_text(text, TypeFace::Prose, color, row),
            Cell::Path(path) => self.path(&path, color, row),
            Cell::Time { at, format } => {
                self.copyable_figures(format.format(&at, self.locale), color, row)
            }
            Cell::Duration(length) => {
                self.copyable_figures(duration_text(length, self.locale), color, row)
            }
            Cell::RelativeMoment(at) => {
                self.copyable_figures(relative_moment_text(at, self.now, self.locale), color, row)
            }
            Cell::SignedDuration(offset) => {
                self.copyable_figures(signed_duration_text(offset, self.locale), color, row)
            }
            Cell::Readout(readout) => self.copyable_figures(readout.text(self.locale), color, row),
            Cell::Quantity(quantity) => {
                self.copyable_figures(quantity.localized_text(self.locale), color, row)
            }
            Cell::Link { label, url, glyph } => self.openable_link(label, url, glyph, color, row),
            Cell::Hint(text) => self.text(text, TypeFace::Prose, self.hint, row),
        }
    }

    fn openable_link(
        &self,
        label: String,
        url: String,
        glyph: Option<Glyph>,
        color: Color,
        link_cell: &Cell,
    ) -> Element<'static, PopupMessage> {
        let underlined_label: Span<'static> = span(label.clone())
            .font(FontFaces::from(&self.prose_font).regular)
            .color(color)
            .underline(true);
        let underlined = container(
            rich_text([underlined_label])
                .size(self.prose_font.text_size_px)
                .line_height(LINE_HEIGHT)
                .wrapping(self.wrapping),
        )
        .max_width(self.max_width);
        let value =
            TransitioningText::new(underlined, label, self.transition, &self.prose_font, color)
                .identified_by(link_cell);
        let branded_value: Element<'static, PopupMessage> = match glyph {
            Some(brand_glyph) => row([self.glyph(brand_glyph, color), value.into()])
                .spacing(GESTURE_ICON_GAP_PX)
                .align_y(Vertical::Center)
                .into(),
            None => value.into(),
        };
        pressable_text(
            branded_value,
            url,
            PopupMessage::OpenUrl,
            self.link,
            self.copy_flash,
            Interaction::Pointer,
        )
    }

    fn glyph(&self, icon: Glyph, color: Color) -> Element<'static, PopupMessage> {
        icon.view(self.prose_font.text_size_px, color)
    }

    fn copyable_figures(
        &self,
        text: String,
        color: Color,
        row: &Cell,
    ) -> Element<'static, PopupMessage> {
        self.copyable_text(text, TypeFace::Figures, color, row)
    }

    fn copyable_text(
        &self,
        text: String,
        face: TypeFace,
        color: Color,
        row: &Cell,
    ) -> Element<'static, PopupMessage> {
        let value = self.text(text.clone(), face, color, row);
        if text.trim().is_empty() {
            return value;
        }
        pressable_text(
            value,
            text,
            PopupMessage::CopyText,
            self.flash,
            self.copy_flash,
            Interaction::Copy,
        )
    }
}

#[derive(Clone, Copy)]
enum TypeFace {
    Prose,
    Figures,
}

fn cell(
    font: &ResolvedFont,
    text: String,
    color: Color,
    max_width: f32,
    wrapping: Wrapping,
) -> Container<'static, PopupMessage> {
    container(bar_text(font, text).color(color).wrapping(wrapping)).max_width(max_width)
}
