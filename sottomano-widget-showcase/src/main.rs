use std::{
    env,
    error::Error,
    iter,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Instant,
};

use clap::Parser;
use sottomano_desktop_sources::system_protocol_clients::shell_command_output::shown_process_text;
use sottomano_toml_config::Config;
use sottomano_widget_showcase::{
    WORKSPACE_ROOT,
    animation_recording::RecordingSession,
    demo_script::{DEMOS, Demo},
    showcase_program::{ShowcaseProgram, booting_clock},
};

const LAVAPIPE_THREADS_VARIABLE: &str = "LP_NUM_THREADS";
const LAVAPIPE_THREADS_PER_DEMO: &str = "4";

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
        conflicts_with = "demo",
        default_value = "4",
        help = "How many demos to render at the same time. Each demo buffers its frames, up to \
                several GB."
    )]
    jobs: NonZeroUsize,
    #[arg(
        long,
        default_value = WORKSPACE_ROOT,
        help = "Directory to write recordings/ into."
    )]
    output: PathBuf,
}

fn render_single_demo(demo_name: &str, output: &Path) -> Result<(), Box<dyn Error>> {
    let demo = DEMOS
        .iter()
        .find(|demo| demo.name == demo_name)
        .ok_or_else(|| format!("no demo is named {demo_name}"))?;
    let start = booting_clock();
    RecordingSession::open(start).record(demo, start, output);
    Ok(())
}

fn render_in_child_process(executable: &Path, demo: &Demo, output: &Path) -> Result<(), String> {
    let child_output = Command::new(executable)
        .arg(demo.name)
        .arg("--output")
        .arg(output)
        .env(LAVAPIPE_THREADS_VARIABLE, LAVAPIPE_THREADS_PER_DEMO)
        .output()
        .map_err(|spawn_error| spawn_error.to_string())?;
    if child_output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}\n{}",
            child_output.status,
            shown_process_text(&child_output.stderr)
        ))
    }
}

fn render_all_demos(jobs: NonZeroUsize, output: &Path) -> Result<(), Box<dyn Error>> {
    let executable = env::current_exe()?;
    let next_demo_position = AtomicUsize::new(0);
    let finished_demo_count = AtomicUsize::new(0);
    let run_started = Instant::now();
    let render_queued_demos = || {
        iter::from_fn(|| DEMOS.get(next_demo_position.fetch_add(1, Ordering::Relaxed)))
            .filter_map(|demo| {
                let demo_started = Instant::now();
                let render_outcome = render_in_child_process(&executable, demo, output);
                let finished = finished_demo_count.fetch_add(1, Ordering::Relaxed) + 1;
                match render_outcome {
                    Ok(()) => {
                        eprintln!(
                            "[{finished}/{}] {} recorded in {:.1}s",
                            DEMOS.len(),
                            demo.name,
                            seconds_since(demo_started)
                        );
                        None
                    }
                    Err(failure_report) => {
                        eprintln!(
                            "[{finished}/{}] {} failed: {failure_report}",
                            DEMOS.len(),
                            demo.name
                        );
                        Some(demo.name)
                    }
                }
            })
            .collect::<Vec<_>>()
    };
    let failed_demos: Vec<&str> = thread::scope(|scope| {
        let workers: Vec<_> = (0..jobs.get())
            .map(|_| scope.spawn(render_queued_demos))
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("render workers do not panic"))
            .collect()
    });
    if !failed_demos.is_empty() {
        return Err(format!("demos failed to render: {}", failed_demos.join(", ")).into());
    }
    eprintln!(
        "recorded {} demos in {:.0}s",
        DEMOS.len(),
        seconds_since(run_started)
    );
    Ok(())
}

fn seconds_since(started: Instant) -> f32 {
    Instant::now()
        .checked_duration_since(started)
        .expect("the monotonic clock does not run backwards")
        .as_secs_f32()
}

fn main() -> Result<(), Box<dyn Error>> {
    let command_line = CommandLine::parse();
    if command_line.window {
        let program = ShowcaseProgram::interactive(&Config::default());
        return Ok(iced_winit::run(iced_tester::attach(program))?);
    }
    match command_line.demo.as_deref() {
        Some(demo_name) => render_single_demo(demo_name, &command_line.output),
        None => render_all_demos(command_line.jobs, &command_line.output),
    }
}
