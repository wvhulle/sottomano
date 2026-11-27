use std::{iter::once, num::NonZeroUsize, rc::Rc};

use iced::{
    Element, Pixels,
    alignment::{Horizontal, Vertical},
    widget::{Space, rule},
};

use super::{ColumnHeading, GroupedTable, RecordCells, TableColumns, TableRow};
use crate::{
    data_tables::{AutoWidthColumn, SpannableRow, auto_width_column, auto_width_spanning_table},
    theming::{
        LINE_HEIGHT, TextFace, background_strong_color, dotted_rule,
        style_metrics::OUTLINE_WIDTH_PX,
    },
};

const DEFAULT_COLUMN_GAP_PX: f32 = 12.0;
const DEFAULT_ROW_SPACING_PX: f32 = 4.0;

type RecordKeyView<CellValue, RowLevel, Message> =
    Rc<dyn Fn(&RecordCells<CellValue, RowLevel>) -> Element<'static, Message>>;
type RecordValueView<CellValue, RowLevel, Message> =
    Rc<dyn Fn(&RecordCells<CellValue, RowLevel>, usize) -> Element<'static, Message>>;

type OmittedCountView<Message> = Rc<dyn Fn(NonZeroUsize) -> Element<'static, Message>>;
type HeadingView<Message> = Rc<dyn Fn(&str) -> Element<'static, Message>>;

pub struct GroupedTableStyle<CellValue, RowLevel, Message> {
    pub key_cell: RecordKeyView<CellValue, RowLevel, Message>,
    pub value_cell: RecordValueView<CellValue, RowLevel, Message>,
    pub omitted_count: OmittedCountView<Message>,
    pub heading: HeadingView<Message>,
    pub text_size_px: f32,
    pub column_gap_px: f32,
    pub row_spacing_px: f32,
}

impl<RowLevel: 'static, Message: 'static> From<TextFace>
    for GroupedTableStyle<String, RowLevel, Message>
{
    fn from(face: TextFace) -> Self {
        Self {
            key_cell: Rc::new(move |record| face.text(record.key.clone()).into()),
            value_cell: Rc::new(move |record, position| {
                face.text(record.cells.get(position).cloned().unwrap_or_default())
                    .into()
            }),
            omitted_count: Rc::new(move |omitted_count| {
                face.text(format!("{omitted_count} more")).into()
            }),
            heading: Rc::new(move |label| face.text(label.to_owned()).into()),
            text_size_px: face.size_px,
            column_gap_px: DEFAULT_COLUMN_GAP_PX,
            row_spacing_px: DEFAULT_ROW_SPACING_PX,
        }
    }
}

enum ColumnedRow<CellValue, RowLevel> {
    Record(Rc<RecordCells<CellValue, RowLevel>>),
    OmittedRecordCount(NonZeroUsize),
    ContinuationMarker,
}

impl<CellValue, RowLevel> Clone for ColumnedRow<CellValue, RowLevel> {
    fn clone(&self) -> Self {
        match self {
            ColumnedRow::Record(record) => ColumnedRow::Record(Rc::clone(record)),
            ColumnedRow::OmittedRecordCount(omitted_count) => {
                ColumnedRow::OmittedRecordCount(*omitted_count)
            }
            ColumnedRow::ContinuationMarker => ColumnedRow::ContinuationMarker,
        }
    }
}

pub fn table_separator<'view, Message: 'view>() -> Element<'view, Message> {
    rule::horizontal(OUTLINE_WIDTH_PX)
        .style(rule::default)
        .into()
}

impl<CellValue: Clone + 'static, RowLevel: Clone + 'static> GroupedTable<'_, CellValue, RowLevel> {
    #[must_use]
    pub fn view<Message: 'static>(
        &self,
        style: &GroupedTableStyle<CellValue, RowLevel, Message>,
    ) -> Element<'static, Message> {
        let line_height_px = LINE_HEIGHT.to_absolute(Pixels(style.text_size_px)).0;
        let value_columns = self
            .columns
            .headings()
            .into_iter()
            .enumerate()
            .map(|(position, heading)| value_column(style, position, heading, line_height_px));
        auto_width_spanning_table(
            once(key_column(
                style,
                self.columns.key_alignment(),
                line_height_px,
            ))
            .chain(value_columns),
            self.rows.iter().flat_map(spannable_rows),
        )
        .column_gap(style.column_gap_px)
        .row_gap(style.row_spacing_px)
        .into()
    }
}

fn key_column<CellValue: 'static, RowLevel: 'static, Message: 'static>(
    style: &GroupedTableStyle<CellValue, RowLevel, Message>,
    key_alignment: Horizontal,
    line_height_px: f32,
) -> AutoWidthColumn<'static, ColumnedRow<CellValue, RowLevel>, Message> {
    let key_cell = Rc::clone(&style.key_cell);
    let omitted_count = Rc::clone(&style.omitted_count);
    auto_width_column(Space::new(), move |content| match content {
        ColumnedRow::Record(record) => key_cell(&record),
        ColumnedRow::OmittedRecordCount(omitted) => omitted_count(omitted),
        ColumnedRow::ContinuationMarker => {
            dotted_rule(background_strong_color, line_height_px).into()
        }
    })
    .align_x(key_alignment)
    .align_y(Vertical::Top)
}

fn value_column<CellValue: 'static, RowLevel: 'static, Message: 'static>(
    style: &GroupedTableStyle<CellValue, RowLevel, Message>,
    position: usize,
    heading: ColumnHeading<'_>,
    line_height_px: f32,
) -> AutoWidthColumn<'static, ColumnedRow<CellValue, RowLevel>, Message> {
    let heading_view: Element<'static, Message> = match heading {
        ColumnHeading::Titled { label, .. } => (style.heading)(label),
        ColumnHeading::Untitled { .. } => Space::new().into(),
    };
    let value_cell = Rc::clone(&style.value_cell);
    auto_width_column(heading_view, move |content| match content {
        ColumnedRow::Record(record) => value_cell(&record, position),
        ColumnedRow::OmittedRecordCount(_) | ColumnedRow::ContinuationMarker => {
            dotted_rule(background_strong_color, line_height_px).into()
        }
    })
    .align_x(heading.alignment())
    .align_y(Vertical::Top)
}

fn spannable_rows<CellValue: Clone + 'static, RowLevel: Clone + 'static, Message: 'static>(
    row: &TableRow<CellValue, RowLevel>,
) -> Vec<SpannableRow<'static, ColumnedRow<CellValue, RowLevel>, Message>> {
    match row {
        TableRow::Record(record) => {
            vec![SpannableRow::Columned(ColumnedRow::Record(Rc::new(
                record.clone(),
            )))]
        }
        TableRow::GroupSeparator => vec![SpannableRow::FullWidth(table_separator())],
        TableRow::TotalRecord(record) => vec![
            SpannableRow::FullWidth(table_separator()),
            SpannableRow::Columned(ColumnedRow::Record(Rc::new(record.clone()))),
        ],
        TableRow::ContinuationMarker => {
            vec![SpannableRow::Columned(ColumnedRow::ContinuationMarker)]
        }
        TableRow::OmittedRecordCount(omitted_count) => vec![SpannableRow::Columned(
            ColumnedRow::OmittedRecordCount(*omitted_count),
        )],
    }
}

pub fn text_grouped_table<'label, Message: 'static>(
    headings: impl IntoIterator<Item = &'label str>,
    rows: impl IntoIterator<Item = (String, Vec<String>)>,
) -> Element<'static, Message> {
    let columns = TableColumns::Headed(headings.into_iter().map(ColumnHeading::titled).collect());
    GroupedTable::new(
        columns,
        rows.into_iter()
            .map(|(key, cells)| RecordCells::new(key, cells, ())),
    )
    .view(&GroupedTableStyle::from(TextFace::default()))
}
