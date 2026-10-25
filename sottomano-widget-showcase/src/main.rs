use std::error::Error;

use clap::Parser;
use sottomano_config::Config;
use sottomano_widget_showcase::{
    animation_recording,
    demo_script::DEMOS,
    showcase_program::{ShowcaseProgram, booting_clock},
    tour_document_embedding,
};

#[derive(Debug, Parser)]
#[command(
    name = "sottomano-widget-showcase",
    about = "Render the tour recordings of sottomano into recordings/."
)]
struct CommandLine {
    #[arg(help = "Render only this demo and skip the tour embedding check.")]
    demo: Option<String>,
    #[arg(
        long,
        conflicts_with = "demo",
        help = "Open the demo fixtures in a window with the iced tester instead of recording."
    )]
    window: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let command_line = CommandLine::parse();
    if command_line.window {
        let program = ShowcaseProgram::interactive(&Config::fallback());
        return Ok(iced_winit::run(iced_tester::attach(program))?);
    }
    let start = booting_clock();
    let only = command_line.demo;
    let recorded: Vec<_> = DEMOS
        .iter()
        .filter(|demo| only.as_deref().is_none_or(|name| name == demo.name))
        .map(|demo| animation_recording::record(demo, start))
        .collect();
    if only.is_none() {
        tour_document_embedding::assert_embedded(&recorded);
    }
    Ok(())
}
