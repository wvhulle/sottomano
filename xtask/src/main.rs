mod icon_sets;
mod recording_embed_widths;
mod recordings_branch;

use std::{error::Error, fmt, fs, path::Path, process::ExitCode};

use clap::{Parser, Subcommand};
use sottomano_toml_config::default_config_toml;

const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const CONFIGURATION_DOCS: &str = "docs/configuration.md";

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Repository maintenance tasks for sottomano.")]
struct XtaskCommandLine {
    #[command(subcommand)]
    task: MaintenanceTask,
}

#[derive(Debug, Subcommand)]
enum MaintenanceTask {
    #[command(about = "Regenerate the full default config section of docs/configuration.md.")]
    UpdateConfigDocs {
        #[arg(
            long,
            help = "Fail when the section is stale instead of rewriting the file."
        )]
        check: bool,
    },
    #[command(about = "Set the width of every recording embedded in README.md and \
                       docs/guide/*.md to its logical width.")]
    UpdateRecordingEmbeds {
        #[arg(
            long,
            help = "Fail when an embed width is stale instead of rewriting the documents."
        )]
        check: bool,
    },
    #[command(
        about = "Force-push the rendered recordings/ directory as the single commit of the \n                 recordings branch on Codeberg, unless its tree is already published."
    )]
    PublishRecordings,
    #[command(
        about = "Fetch the three icon sets at the revisions the flake pins and print the \n                 PHOSPHOR_SVG, SIMPLE_ICONS_SVG and WEATHER_ICONS_SVG exports for eval."
    )]
    FetchIconSvgs,
}

struct GeneratedSection {
    start_marker: &'static str,
    end_marker: &'static str,
}

const DEFAULT_CONFIG_SECTION: GeneratedSection = GeneratedSection {
    start_marker: "<!-- default-config:start -->",
    end_marker: "<!-- default-config:end -->",
};

#[derive(Debug)]
enum SectionSpliceError {
    MarkerMissing(&'static str),
    MarkerRepeated(&'static str),
    MarkersReversed,
}

impl fmt::Display for SectionSpliceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SectionSpliceError::MarkerMissing(marker) => {
                write!(formatter, "{CONFIGURATION_DOCS} has no `{marker}` line")
            }
            SectionSpliceError::MarkerRepeated(marker) => {
                write!(
                    formatter,
                    "{CONFIGURATION_DOCS} has `{marker}` more than once"
                )
            }
            SectionSpliceError::MarkersReversed => write!(
                formatter,
                "{CONFIGURATION_DOCS} closes the generated section before opening it"
            ),
        }
    }
}

impl Error for SectionSpliceError {}

impl GeneratedSection {
    fn unique_offset(markdown: &str, marker: &'static str) -> Result<usize, SectionSpliceError> {
        let offset = markdown
            .find(marker)
            .ok_or(SectionSpliceError::MarkerMissing(marker))?;
        match markdown.matches(marker).count() {
            1 => Ok(offset),
            _ => Err(SectionSpliceError::MarkerRepeated(marker)),
        }
    }

    fn spliced_markdown(
        &self,
        markdown: &str,
        generated_body: &str,
    ) -> Result<String, SectionSpliceError> {
        let content_start =
            Self::unique_offset(markdown, self.start_marker)? + self.start_marker.len();
        let content_end = Self::unique_offset(markdown, self.end_marker)?;
        if content_end < content_start {
            return Err(SectionSpliceError::MarkersReversed);
        }
        Ok(format!(
            "{}\n{}\n{}",
            &markdown[..content_start],
            generated_body.trim_end(),
            &markdown[content_end..]
        ))
    }
}

fn fenced_default_config() -> Result<String, Box<dyn Error>> {
    Ok(format!(
        "```toml\n{}\n```",
        default_config_toml()?.trim_end()
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocsMode {
    Check,
    Write,
}

fn update_config_docs(docs_mode: DocsMode) -> Result<ExitCode, Box<dyn Error>> {
    let docs_path = Path::new(WORKSPACE_ROOT).join(CONFIGURATION_DOCS);
    let current_markdown = fs::read_to_string(&docs_path)?;
    let updated_markdown =
        DEFAULT_CONFIG_SECTION.spliced_markdown(&current_markdown, &fenced_default_config()?)?;
    if updated_markdown == current_markdown {
        println!("{CONFIGURATION_DOCS} already shows the current default config");
        return Ok(ExitCode::SUCCESS);
    }
    if docs_mode == DocsMode::Check {
        eprintln!(
            "{CONFIGURATION_DOCS} shows an outdated default config; run `cargo xtask \
             update-config-docs`"
        );
        return Ok(ExitCode::FAILURE);
    }
    fs::write(&docs_path, updated_markdown)?;
    println!("updated the default config in {CONFIGURATION_DOCS}");
    Ok(ExitCode::SUCCESS)
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let docs_mode = |check_flag: bool| {
        if check_flag {
            DocsMode::Check
        } else {
            DocsMode::Write
        }
    };
    match XtaskCommandLine::parse().task {
        MaintenanceTask::UpdateConfigDocs { check } => update_config_docs(docs_mode(check)),
        MaintenanceTask::UpdateRecordingEmbeds { check } => {
            recording_embed_widths::update_recording_embeds(
                Path::new(WORKSPACE_ROOT),
                docs_mode(check),
            )
        }
        MaintenanceTask::PublishRecordings => {
            recordings_branch::publish_recordings(Path::new(WORKSPACE_ROOT))
        }
        MaintenanceTask::FetchIconSvgs => icon_sets::fetch_icon_sets(),
    }
}
