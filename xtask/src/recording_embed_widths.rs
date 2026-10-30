use std::{
    error::Error,
    fs, io,
    iter::once,
    path::{Path, PathBuf},
    process::ExitCode,
};

use image::ImageReader;

const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDING_EXTENSION: &str = "webp";
const RECORDING_PIXELS_PER_LOGICAL_PIXEL: u32 = 2;
const WIDTH_ATTRIBUTE: &str = r#" width=""#;

const README_DOCUMENT: &str = "README.md";
const GUIDE_DIRECTORY: &str = "docs/guide";
const GUIDE_RECORDINGS_PREFIX: &str = "../../";
const MARKDOWN_EXTENSION: &str = "md";

struct TourDocument {
    path: PathBuf,
    recordings_prefix: &'static str,
}

fn tour_documents(workspace_root: &Path) -> io::Result<Vec<TourDocument>> {
    let mut guide_pages: Vec<PathBuf> = fs::read_dir(workspace_root.join(GUIDE_DIRECTORY))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == MARKDOWN_EXTENSION)
        })
        .filter_map(|path| {
            path.strip_prefix(workspace_root)
                .ok()
                .map(Path::to_path_buf)
        })
        .collect();
    guide_pages.sort();
    Ok(once(TourDocument {
        path: PathBuf::from(README_DOCUMENT),
        recordings_prefix: "",
    })
    .chain(guide_pages.into_iter().map(|path| TourDocument {
        path,
        recordings_prefix: GUIDE_RECORDINGS_PREFIX,
    }))
    .collect())
}

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
    let tour = tour_documents(workspace_root)?;
    let documents: Vec<(&TourDocument, String)> = tour
        .iter()
        .map(|document| {
            Ok((
                document,
                fs::read_to_string(workspace_root.join(&document.path))?,
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
                format!(
                    "{}: {}",
                    document.path.display(),
                    document.embedding(recording)
                )
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
                document.path.display()
            );
        });
        return Ok(ExitCode::FAILURE);
    }
    stale_documents.iter().try_for_each(|(document, resized)| {
        fs::write(workspace_root.join(&document.path), resized)
    })?;
    stale_documents.iter().for_each(|(document, _)| {
        println!("updated recording widths in {}", document.path.display());
    });
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf, process};

    use super::{tour_documents, with_logical_width};

    #[test]
    fn rewrites_only_the_width_of_the_named_recording() {
        let markdown =
            r#"<img src="recordings/a.webp" width="10"> <img src="recordings/b.webp" width="20">"#;
        assert_eq!(
            with_logical_width(markdown, r#"<img src="recordings/a.webp""#, 42),
            r#"<img src="recordings/a.webp" width="42"> <img src="recordings/b.webp" width="20">"#
        );
    }

    #[test]
    fn guide_pages_follow_the_readme_with_their_deeper_prefix() {
        let workspace = env::temp_dir().join(format!("sottomano-tour-{}", process::id()));
        fs::create_dir_all(workspace.join("docs/guide")).unwrap();
        fs::write(workspace.join("docs/guide/weather.md"), "").unwrap();
        fs::write(workspace.join("docs/guide/compute.md"), "").unwrap();
        fs::write(workspace.join("docs/guide/notes.txt"), "").unwrap();
        let documents = tour_documents(&workspace).unwrap();
        fs::remove_dir_all(&workspace).unwrap();
        let listed: Vec<(PathBuf, &str)> = documents
            .into_iter()
            .map(|document| (document.path, document.recordings_prefix))
            .collect();
        assert_eq!(
            listed,
            [
                (PathBuf::from("README.md"), ""),
                (PathBuf::from("docs/guide/compute.md"), "../../"),
                (PathBuf::from("docs/guide/weather.md"), "../../"),
            ]
        );
    }
}
