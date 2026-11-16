pub mod hover_card_figure_payloads;
pub mod hover_card_source_delivery;

use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    rc::Rc,
};

use corredo::data_tables::GroupedTable;
use iced::{Element, alignment::Horizontal};
use sottomano_desktop_sources::SnapshotSource;
use sottomano_toml_config::severity::Severity;

use self::hover_card_source_delivery::{SectionSource, SourceDelivery};
use crate::{
    hover_cards::{PopupMessage, PressAction, hover_card_content::HoverCardContent},
    widgets::{
        keyed_record_table::{
            ColumnHeading, TableColumns, TableRecord, column_record_table, grouped_record_table,
        },
        value_cells::{LabeledValue, ValueCell},
    },
};

pub type SectionTable = GroupedTable<'static, ValueCell, Severity>;

type DrawSection = dyn Fn(&HoverCardContent<'_>) -> Element<'static, PopupMessage>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WidgetContent {
    Filled,
    Empty,
}

#[derive(Clone)]
pub struct SectionWidget {
    draw: Rc<DrawSection>,
    content: WidgetContent,
}

impl SectionWidget {
    pub fn drawn(
        draw: impl Fn(&HoverCardContent<'_>) -> Element<'static, PopupMessage> + 'static,
    ) -> Self {
        Self {
            draw: Rc::new(draw),
            content: WidgetContent::Filled,
        }
    }

    pub fn paragraph(text: impl Into<String>) -> Self {
        let text = text.into();
        let content = if text.is_empty() {
            WidgetContent::Empty
        } else {
            WidgetContent::Filled
        };
        Self {
            draw: Rc::new(move |card: &HoverCardContent<'_>| card.paragraph(text.clone())),
            content,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content == WidgetContent::Empty
    }

    pub(crate) fn draw(&self, card: &HoverCardContent<'_>) -> Element<'static, PopupMessage> {
        (self.draw)(card)
    }
}

impl From<SectionTable> for SectionWidget {
    fn from(table: SectionTable) -> Self {
        let content = if table.is_empty() {
            WidgetContent::Empty
        } else {
            WidgetContent::Filled
        };
        Self {
            draw: Rc::new(move |card: &HoverCardContent<'_>| {
                table.view(&card.record_table_style())
            }),
            content,
        }
    }
}

impl Debug for SectionWidget {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("SectionWidget")
            .field("content", &self.content)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionTitle {
    Bare(String),
    Captioned { title: String, caption: String },
}

impl SectionTitle {
    pub fn captioned(title: impl Into<String>, caption: impl Into<String>) -> Self {
        SectionTitle::Captioned {
            title: title.into(),
            caption: caption.into(),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            SectionTitle::Bare(title) | SectionTitle::Captioned { title, .. } => title,
        }
    }
}

impl From<&str> for SectionTitle {
    fn from(title: &str) -> Self {
        SectionTitle::Bare(title.to_owned())
    }
}

impl From<String> for SectionTitle {
    fn from(title: String) -> Self {
        SectionTitle::Bare(title)
    }
}

#[derive(Debug, Clone)]
pub struct Section {
    title: SectionTitle,
    widget: SectionWidget,
    source: SectionSource,
}

impl Section {
    pub fn new(title: impl Into<SectionTitle>, widget: impl Into<SectionWidget>) -> Self {
        Self {
            title: title.into(),
            widget: widget.into(),
            source: SectionSource::Local,
        }
    }

    pub(crate) fn stamped_by<S: SnapshotSource>(self, delivery: SourceDelivery) -> Self {
        Self {
            source: SectionSource::Snapshot {
                source_name: S::NAME,
                delivery,
            },
            ..self
        }
    }

    #[must_use]
    pub fn press_actions(mut actions: Vec<PressAction>) -> Option<Self> {
        if actions.is_empty() {
            return None;
        }
        actions.sort_by_key(|action| action.gesture);
        Some(Self::new(
            "Actions",
            SectionWidget::drawn(move |card| card.press_actions_table(&actions)),
        ))
    }

    pub(crate) fn source_state<S: SnapshotSource>(
        delivery: SourceDelivery,
        detail: impl Into<String>,
    ) -> Self {
        let title = match delivery {
            SourceDelivery::Delivered(_) | SourceDelivery::Loading => "Loading",
            SourceDelivery::Failed => "Unavailable",
            SourceDelivery::Offline => "Offline",
        };
        let widget = match delivery {
            SourceDelivery::Loading => {
                SectionWidget::drawn(|card: &HoverCardContent<'_>| card.loading_placeholder())
            }
            SourceDelivery::Delivered(_) | SourceDelivery::Failed | SourceDelivery::Offline => {
                SectionWidget::paragraph(detail)
            }
        };
        Self::new(title, widget).stamped_by::<S>(delivery)
    }

    #[must_use]
    pub fn source(&self) -> SectionSource {
        self.source
    }

    #[deprecated(note = "every section needs a title")]
    pub fn untitled(widget: impl Into<SectionWidget>) -> Self {
        Self::new("", widget)
    }

    #[must_use]
    pub fn title(&self) -> &SectionTitle {
        &self.title
    }

    #[must_use]
    pub fn widget(&self) -> &SectionWidget {
        &self.widget
    }
}

pub fn dictionary_table(entries: impl IntoIterator<Item = LabeledValue>) -> SectionTable {
    column_record_table(TableColumns::Unheaded(vec![Horizontal::Left]), entries)
}

pub fn headed_table<const COLUMNS: usize>(
    headings: [ColumnHeading; COLUMNS],
    records: impl IntoIterator<Item: Into<TableRecord<COLUMNS>>>,
) -> SectionTable {
    column_record_table(TableColumns::Headed(headings.into()), records)
}

pub fn headed_grouped_table<const COLUMNS: usize>(
    headings: [ColumnHeading; COLUMNS],
    groups: impl IntoIterator<Item: IntoIterator<Item: Into<TableRecord<COLUMNS>>>>,
) -> SectionTable {
    grouped_record_table(TableColumns::Headed(headings.into()), groups)
}
