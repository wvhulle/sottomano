mod demo_script;
mod fixtures;
mod gif_recording;
mod hovered_widget;
mod readme_embedding;
mod showcase_program;
mod showcased_group;

use clap::Parser;

const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const RECORDINGS_DIRECTORY: &str = "recordings";

#[derive(Debug, Parser)]
#[command(
    name = "vetrata-readme-showcase",
    about = "Render the README recordings of vetrata into recordings/."
)]
struct CommandLine {
    #[arg(help = "Render only this demo and skip the README embedding check.")]
    demo: Option<String>,
}

fn main() {
    let only = CommandLine::parse().demo;
    let recorded: Vec<_> = demo_script::DEMOS
        .iter()
        .filter(|demo| only.as_deref().is_none_or(|name| name == demo.name))
        .map(gif_recording::record)
        .collect();
    if only.is_none() {
        readme_embedding::assert_embedded(&recorded);
    }
}
