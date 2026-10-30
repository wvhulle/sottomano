use std::{error::Error, path::PathBuf, time::Instant};

use clap::Parser;
use sottomano_config::Config;
use sottomano_widget_showcase::{
    WORKSPACE_ROOT,
    animation_recording::RecordingSession,
    demo_script::{DEMOS, RecordedLook},
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
        let program = ShowcaseProgram::interactive(&Config::default());
        return Ok(iced_winit::run(iced_tester::attach(program))?);
    }
    let start = booting_clock();
    let only = command_line.demo.as_deref();
    let selected_demos: Vec<_> = DEMOS
        .iter()
        .filter(|demo| only.is_none_or(|name| name == demo.name))
        .collect();
    let mut recording_session = RecordingSession::open(start);
    let run_started = Instant::now();
    selected_demos
        .iter()
        .enumerate()
        .for_each(|(demo_position, demo)| {
            let demo_started = Instant::now();
            RecordedLook::recorded_for(demo).for_each(|look| {
                recording_session.record(demo, look, start, &command_line.output);
            });
            eprintln!(
                "[{}/{}] {} recorded in {:.1}s",
                demo_position + 1,
                selected_demos.len(),
                demo.name,
                demo_started.elapsed().as_secs_f32()
            );
        });
    eprintln!(
        "recorded {} demos in {:.0}s",
        selected_demos.len(),
        run_started.elapsed().as_secs_f32()
    );
    Ok(())
}
