use std::{fs, path::Path};

use crate::{RECORDING_EXTENSION, RECORDINGS_DIRECTORY, WORKSPACE_ROOT};

pub struct RecordedAnimation {
    pub name: &'static str,
    pub width: u16,
}

struct TourDocument {
    path: &'static str,
    recordings_prefix: &'static str,
}

const TOUR_DOCUMENTS: &[TourDocument] = &[
    TourDocument {
        path: "README.md",
        recordings_prefix: "",
    },
    TourDocument {
        path: "docs/showcase.md",
        recordings_prefix: "../",
    },
];

struct LoadedTourDocument {
    document: &'static TourDocument,
    markdown: String,
}

impl LoadedTourDocument {
    fn load(document: &'static TourDocument) -> Self {
        let markdown = fs::read_to_string(Path::new(WORKSPACE_ROOT).join(document.path))
            .unwrap_or_else(|_| panic!("{} is readable", document.path));
        LoadedTourDocument { document, markdown }
    }

    fn recording_source(&self, recorded: &RecordedAnimation) -> String {
        format!(
            r#"<img src="{}{RECORDINGS_DIRECTORY}/{}.{RECORDING_EXTENSION}""#,
            self.document.recordings_prefix, recorded.name
        )
    }

    fn embedding(&self, recorded: &RecordedAnimation) -> String {
        format!(
            r#"{} width="{}""#,
            self.recording_source(recorded),
            recorded.width
        )
    }

    fn embeds(&self, recorded: &RecordedAnimation) -> bool {
        self.markdown
            .contains(self.recording_source(recorded).as_str())
    }

    fn embeds_at_logical_width(&self, recorded: &RecordedAnimation) -> bool {
        self.markdown.contains(self.embedding(recorded).as_str())
    }
}

fn misembedded(recorded: &RecordedAnimation, documents: &[LoadedTourDocument]) -> Vec<String> {
    let embedding_documents: Vec<&LoadedTourDocument> = documents
        .iter()
        .filter(|document| document.embeds(recorded))
        .collect();
    if embedding_documents.is_empty() {
        return documents
            .iter()
            .map(|document| {
                format!(
                    "{}: {}",
                    document.document.path,
                    document.embedding(recorded)
                )
            })
            .collect();
    }
    embedding_documents
        .into_iter()
        .filter(|document| !document.embeds_at_logical_width(recorded))
        .map(|document| {
            format!(
                "{}: {}",
                document.document.path,
                document.embedding(recorded)
            )
        })
        .collect()
}

pub fn assert_embedded(recorded: &[RecordedAnimation]) {
    let documents: Vec<LoadedTourDocument> = TOUR_DOCUMENTS
        .iter()
        .map(LoadedTourDocument::load)
        .collect();
    let misembedded: Vec<String> = recorded
        .iter()
        .flat_map(|animation| misembedded(animation, &documents))
        .collect();
    assert!(
        misembedded.is_empty(),
        "embed each recording at its logical width in one of the tour documents:\n{}",
        misembedded.join("\n")
    );
}
