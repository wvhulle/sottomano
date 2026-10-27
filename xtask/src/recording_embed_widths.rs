use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::ExitCode,
};

use image::ImageReader;

const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDING_EXTENSION: &str = "webp";
const RECORDING_PIXELS_PER_LOGICAL_PIXEL: u32 = 2;
const WIDTH_ATTRIBUTE: &str = r#" width=""#;

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

struct RenderedRecording {
    name: String,
    logical_width: u32,
}

impl TourDocument {
    fn recording_source(&self, recording: &RenderedRecording) -> String {
        format!(
            r#"<img src="{}{RECORDINGS_DIRECTORY}/{}.{RECORDING_EXTENSION}""#,
            self.recordings_prefix, recording.name
        )
    }

    fn embedding(&self, recording: &RenderedRecording) -> String {
        format!(
            r#"{}{WIDTH_ATTRIBUTE}{}""#,
            self.recording_source(recording),
            recording.logical_width
        )
    }
}

fn rendered_recordings(workspace_root: &Path) -> Result<Vec<RenderedRecording>, Box<dyn Error>> {
    let mut recording_paths: Vec<PathBuf> =
        fs::read_dir(workspace_root.join(RECORDINGS_DIRECTORY))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == RECORDING_EXTENSION)
            })
            .collect();
    recording_paths.sort();
    recording_paths
        .iter()
        .map(|path| {
            let (pixel_width, _) = ImageReader::open(path)?.into_dimensions()?;
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| format!("{} has no readable name", path.display()))?
                .to_owned();
            Ok(RenderedRecording {
                name,
                logical_width: pixel_width / RECORDING_PIXELS_PER_LOGICAL_PIXEL,
            })
        })
        .collect()
}

fn with_logical_width(markdown: &str, recording_source: &str, width: u32) -> String {
    let mut pieces = markdown.split(recording_source);
    let leading = pieces.next().unwrap_or_default().to_owned();
    pieces.fold(leading, |resized, following| {
        let after_width = following
            .strip_prefix(WIDTH_ATTRIBUTE)
            .and_then(|attribute_value| {
                attribute_value
                    .find('"')
                    .map(|closing_quote| &attribute_value[closing_quote + 1..])
            });
        match after_width {
            Some(remainder) => {
                format!(r#"{resized}{recording_source}{WIDTH_ATTRIBUTE}{width}"{remainder}"#)
            }
            None => format!("{resized}{recording_source}{following}"),
        }
    })
}

pub fn update_recording_embeds(
    workspace_root: &Path,
    check: bool,
) -> Result<ExitCode, Box<dyn Error>> {
    let recordings = rendered_recordings(workspace_root)?;
    let documents: Vec<(&TourDocument, String)> = TOUR_DOCUMENTS
        .iter()
        .map(|document| {
            Ok((
                document,
                fs::read_to_string(workspace_root.join(document.path))?,
            ))
        })
        .collect::<Result<_, io::Error>>()?;
    let unembedded: Vec<String> = recordings
        .iter()
        .filter(|recording| {
            !documents.iter().any(|(document, markdown)| {
                markdown.contains(&document.recording_source(recording))
            })
        })
        .flat_map(|recording| {
            documents.iter().map(move |(document, _)| {
                format!("{}: {}", document.path, document.embedding(recording))
            })
        })
        .collect();
    if !unembedded.is_empty() {
        eprintln!(
            "embed each recording in one of the tour documents:\n{}",
            unembedded.join("\n")
        );
        return Ok(ExitCode::FAILURE);
    }
    let stale_documents: Vec<(&TourDocument, String)> = documents
        .into_iter()
        .filter_map(|(document, markdown)| {
            let resized = recordings
                .iter()
                .fold(markdown.clone(), |resized, recording| {
                    with_logical_width(
                        &resized,
                        &document.recording_source(recording),
                        recording.logical_width,
                    )
                });
            (resized != markdown).then_some((document, resized))
        })
        .collect();
    if stale_documents.is_empty() {
        println!("the tour documents embed every recording at its logical width");
        return Ok(ExitCode::SUCCESS);
    }
    if check {
        stale_documents.iter().for_each(|(document, _)| {
            eprintln!(
                "{} embeds recordings at outdated widths; run `cargo xtask \
                 update-recording-embeds`",
                document.path
            );
        });
        return Ok(ExitCode::FAILURE);
    }
    stale_documents.iter().try_for_each(|(document, resized)| {
        fs::write(workspace_root.join(document.path), resized)
    })?;
    stale_documents
        .iter()
        .for_each(|(document, _)| println!("updated recording widths in {}", document.path));
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::with_logical_width;

    #[test]
    fn rewrites_only_the_width_of_the_named_recording() {
        let markdown =
            r#"<img src="recordings/a.webp" width="10"> <img src="recordings/b.webp" width="20">"#;
        assert_eq!(
            with_logical_width(markdown, r#"<img src="recordings/a.webp""#, 42),
            r#"<img src="recordings/a.webp" width="42"> <img src="recordings/b.webp" width="20">"#
        );
    }
}
