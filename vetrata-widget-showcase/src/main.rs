use std::error::Error;

use clap::Parser;
use vetrata_config::Config;
use vetrata_widget_showcase::{
    animation_recording, demo_script::DEMOS, readme_embedding, showcase_program::ShowcaseProgram,
};

#[derive(Debug, Parser)]
#[command(
    name = "vetrata-widget-showcase",
    about = "Render the README recordings of vetrata into recordings/."
)]
struct CommandLine {
    #[arg(help = "Render only this demo and skip the README embedding check.")]
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
        let program = ShowcaseProgram::new(&Config::fallback());
        return Ok(iced_winit::run(iced_tester::attach(program))?);
    }
    let only = command_line.demo;
    let recorded: Vec<_> = DEMOS
        .iter()
        .filter(|demo| only.as_deref().is_none_or(|name| name == demo.name))
        .map(animation_recording::record)
        .collect();
    if only.is_none() {
        readme_embedding::assert_embedded(&recorded);
    }
    Ok(())
}
