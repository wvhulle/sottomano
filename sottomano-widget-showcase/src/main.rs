use std::{error::Error, path::PathBuf};

use clap::Parser;
use sottomano_config::Config;
use sottomano_widget_showcase::{
    WORKSPACE_ROOT, animation_recording,
    demo_script::DEMOS,
    showcase_program::{ShowcaseProgram, booting_clock},
};

#[derive(Debug, Parser)]
#[command(
    name = "sottomano-widget-showcase",
    about = "Render the tour recordings of sottomano into recordings/."
)]
struct CommandLine {
    #[arg(help = "Render only this demo.")]
    demo: Option<String>,
    #[arg(
        long,
        conflicts_with = "demo",
        help = "Open the demo fixtures in a window with the iced tester instead of recording."
    )]
    window: bool,
    #[arg(
        long,
        default_value = WORKSPACE_ROOT,
        help = "Directory to write recordings/ into."
    )]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let command_line = CommandLine::parse();
    if command_line.window {
        let program = ShowcaseProgram::interactive(&Config::fallback());
        return Ok(iced_winit::run(iced_tester::attach(program))?);
    }
    let start = booting_clock();
    let only = command_line.demo.as_deref();
    DEMOS
        .iter()
        .filter(|demo| only.is_none_or(|name| name == demo.name))
        .for_each(|demo| animation_recording::record(demo, start, &command_line.output));
    Ok(())
}
