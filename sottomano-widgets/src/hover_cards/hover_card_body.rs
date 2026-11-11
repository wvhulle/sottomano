pub mod hover_card_figure_payloads;
pub mod hover_card_source_delivery;

use std::{convert::Infallible, fmt::Display, num::NonZeroUsize};

use corredo::record_table::RecordTable;
use sottomano_toml_config::severity::Severity;

use sottomano_desktop_sources::{
    SnapshotSource, SourceCaption, desktop_windows_apps::process_tree_inspection::Process,
};

use self::hover_card_figure_payloads::{ChipletCells, ControllerCallouts, WorkspacePreview};
use crate::{
    clock::Clock,
    widget_groups::window_group::process_tree::Folding,
    widgets::{
        keyed_record_table::{
            ColumnHeading, IntoTableRecord, RecordCells, TableColumns, column_record_table,
            grouped_record_table, labeled_value_record_table, record_cells,
        },
        value_cells::{LabeledValue, ValueCell},
    },
};

pub type SectionTable = RecordTable<'static, ValueCell, Severity>;

pub trait ReadingSections {
    fn sections(&self, clock: Clock) -> Vec<Section>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionBody {
    ColumnTable(SectionTable),
    Tree { root: Process, folding: Folding },
    CoreGrid(Vec<ChipletCells>),
    ControllerDiagram(ControllerCallouts),
    Note { title: String },
    DetailedNote { title: String, body: String },
    SectionSeeding,
    Workspace(WorkspacePreview),
}

impl SectionBody {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            SectionBody::ColumnTable(table) => table.is_empty(),
            SectionBody::CoreGrid(chiplets) => chiplets.is_empty(),
            SectionBody::ControllerDiagram(diagram) => diagram.callouts.is_empty(),
            SectionBody::Workspace(preview) => {
                preview.columns.is_empty() && preview.floating.is_empty()
            }
            SectionBody::Tree { .. }
            | SectionBody::Note { .. }
            | SectionBody::DetailedNote { .. }
            | SectionBody::SectionSeeding => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Heading {
    Plain { title: String },
    Captioned { title: String, caption: String },
}

impl Heading {
    fn of_source<S: SnapshotSource>() -> Self {
        let title = S::NAME.to_owned();
        match S::CAPTION {
            SourceCaption::Captioned(caption) => Heading::Captioned {
                title,
                caption: caption.to_owned(),
            },
            SourceCaption::Uncaptioned => Heading::Plain { title },
        }
    }

    #[must_use]
    pub fn title(&self) -> &str {
        match self {
            Heading::Plain { title } | Heading::Captioned { title, .. } => title,
        }
    }

    #[must_use]
    pub fn caption(&self) -> Option<&str> {
        match self {
            Heading::Plain { .. } => None,
            Heading::Captioned { caption, .. } => Some(caption),
        }
    }
}

impl From<&str> for Heading {
    fn from(title: &str) -> Self {
        title.to_owned().into()
    }
}

impl From<String> for Heading {
    fn from(title: String) -> Self {
        Heading::Plain { title }
    }
}

impl<Title: Into<String>, Caption: Into<String>> From<(Title, Caption)> for Heading {
    fn from((title, caption): (Title, Caption)) -> Self {
        Heading::Captioned {
            title: title.into(),
            caption: caption.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionHeading {
    Untitled,
    Titled(Heading),
}

pub trait ShownAsErrorSection: Display {}

impl<Failure: ShownAsErrorSection> From<Failure> for Section {
    fn from(failure: Failure) -> Self {
        Section::error(failure)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    heading: SectionHeading,
    body: SectionBody,
}

impl Section {
    fn untitled(body: SectionBody) -> Self {
        Self {
            heading: SectionHeading::Untitled,
            body,
        }
    }

    #[must_use]
    pub fn section_seeding() -> Self {
        Self::untitled(SectionBody::SectionSeeding)
    }

    pub fn note(title: impl Into<String>) -> Self {
        Self::untitled(SectionBody::Note {
            title: title.into(),
        })
    }

    pub fn detailed_note(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::untitled(SectionBody::DetailedNote {
            title: title.into(),
            body: body.into(),
        })
    }

    pub fn dictionary(entries: impl IntoIterator<Item = LabeledValue>) -> Self {
        Self::untitled(SectionBody::ColumnTable(labeled_value_record_table(
            entries,
        )))
    }

    pub fn column_table<const COLUMNS: usize>(
        headings: [ColumnHeading; COLUMNS],
        records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
    ) -> Self {
        Self::untitled(SectionBody::ColumnTable(column_record_table(
            TableColumns::Headed(headings.into()),
            records,
        )))
    }

    pub fn grouped_column_table<const COLUMNS: usize>(
        headings: [ColumnHeading; COLUMNS],
        groups: impl IntoIterator<Item: IntoIterator<Item: IntoTableRecord<COLUMNS>>>,
    ) -> Self {
        Self::untitled(SectionBody::ColumnTable(grouped_record_table(
            TableColumns::Headed(headings.into()),
            groups,
        )))
    }

    pub fn error(message: impl Display) -> Self {
        Self::dictionary([LabeledValue::new("Error", message.to_string())])
    }

    fn with_changed_table(self, change: impl FnOnce(SectionTable) -> SectionTable) -> Self {
        match self.body {
            SectionBody::ColumnTable(table) => Self {
                body: SectionBody::ColumnTable(change(table)),
                ..self
            },
            other => Self {
                body: other,
                ..self
            },
        }
    }

    #[must_use]
    pub fn with_entries(self, extra: impl IntoIterator<Item = LabeledValue>) -> Self {
        self.with_changed_table(|table| {
            table.with_appended_records(record_cells::<1>(extra))
        })
    }

    #[must_use]
    pub fn truncated_to(self, row_limit: NonZeroUsize) -> Self {
        self.with_changed_table(|table| table.truncated_to(row_limit))
    }

    #[must_use]
    pub fn without_blank_columns(self) -> Self {
        self.with_changed_table(|table| {
            table.without_blank_columns(|cell| matches!(cell, ValueCell::Blank))
        })
    }

    #[must_use]
    pub fn marked_as_continuing(self) -> Self {
        self.with_changed_table(RecordTable::marked_as_continuing)
    }

    #[must_use]
    pub fn with_total_record<const COLUMNS: usize>(
        self,
        total: impl IntoTableRecord<COLUMNS>,
    ) -> Self {
        self.with_changed_table(|table| {
            table.with_total_record(RecordCells::from(total.into_table_record()))
        })
    }

    #[must_use]
    pub fn tree(root: Option<&Process>, folding: &Folding) -> Self {
        match root {
            Some(root) => Self::untitled(SectionBody::Tree {
                root: root.clone(),
                folding: folding.clone(),
            }),
            None => Self::dictionary([]),
        }
    }

    #[must_use]
    pub fn titled(self, heading: impl Into<Heading>) -> Self {
        Self {
            heading: SectionHeading::Titled(heading.into()),
            ..self
        }
    }

    #[must_use]
    pub fn heading(&self) -> &SectionHeading {
        &self.heading
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        match &self.heading {
            SectionHeading::Untitled => None,
            SectionHeading::Titled(heading) => Some(heading.title()),
        }
    }

    #[must_use]
    pub fn body(&self) -> &SectionBody {
        &self.body
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoverCardBody {
    sections: Vec<Section>,
}

impl HoverCardBody {
    pub fn sections(sections: impl IntoIterator<Item = Section>) -> Self {
        Self {
            sections: sections.into_iter().collect(),
        }
    }

    pub fn dictionary(entries: impl IntoIterator<Item = LabeledValue>) -> Self {
        Self::sections([Section::dictionary(entries)])
    }

    #[must_use]
    pub fn with_sections(mut self, extra: impl IntoIterator<Item = Section>) -> Self {
        self.sections.extend(extra);
        self
    }

    #[must_use]
    pub fn listed_sections(&self) -> &[Section] {
        &self.sections
    }
}

impl From<Section> for HoverCardBody {
    fn from(section: Section) -> Self {
        Self::sections([section])
    }
}

impl From<Infallible> for Section {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}
