use std::{
    env,
    error::Error,
    ffi::OsStr,
    iter,
    num::NonZeroUsize,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Instant,
};

use clap::Parser;
use sottomano_widget_scenes::showcase_program::booting_clock;

use crate::demo_script::{DEMOS, Demo};

mod animation_recording;
mod demo_script;
mod recording_still;

const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDING_EXTENSION: &str = "webp";

const LAVAPIPE_THREADS_VARIABLE: &str = "LP_NUM_THREADS";
const LAVAPIPE_THREADS_PER_DEMO: usize = 4;
const ICED_TEST_BACKEND_VARIABLE: &str = "ICED_TEST_BACKEND";
const RECORDING_BACKEND: &str = "wgpu";
const RENDER_IN_THIS_PROCESS_FLAG: &str = "--render-in-this-process";

#[derive(Debug, Parser)]
#[command(
    name = "sottomano-tour-recordings",
    about = "Render the tour recordings of sottomano into recordings/."
)]
struct CommandLine {
    #[arg(help = "Render only these demos.")]
    demo_names: Vec<String>,
    #[arg(
        long,
        default_value_t = default_render_jobs(),
        help = "How many demos to render at the same time, by default one per four cores. Each \
                demo buffers its frames, up to 2 GB."
    )]
    jobs: NonZeroUsize,
    #[arg(
        long,
        default_value = WORKSPACE_ROOT,
        help = "Directory to write recordings/ into."
    )]
    output: PathBuf,
    #[arg(long, hide = true)]
    render_in_this_process: bool,
}

fn default_render_jobs() -> NonZeroUsize {
    let available_cores =
        thread::available_parallelism().expect("the number of available cores is queryable");
    NonZeroUsize::new(available_cores.get() / LAVAPIPE_THREADS_PER_DEMO)
        .unwrap_or(NonZeroUsize::MIN)
}

fn select_demos(demo_names: &[String]) -> Result<Vec<&'static Demo>, Box<dyn Error>> {
    if let Some(unknown_name) = demo_names
        .iter()
        .find(|demo_name| DEMOS.iter().all(|demo| demo.name != demo_name.as_str()))
    {
        return Err(format!("no demo is named {unknown_name}").into());
    }
    Ok(DEMOS
        .iter()
        .filter(|demo| {
            demo_names.is_empty() || demo_names.iter().any(|demo_name| demo.name == demo_name)
        })
        .collect())
}

fn render_single_demo(demo: &'static Demo, output: &Path) {
    animation_recording::record(demo, booting_clock(), output);
}

fn render_in_child_process(executable: &Path, demo: &Demo, output: &Path) -> Result<(), String> {
    let child_output = Command::new(executable)
        .arg(demo.name)
        .arg("--output")
        .arg(output)
        .arg(RENDER_IN_THIS_PROCESS_FLAG)
        .env(
            LAVAPIPE_THREADS_VARIABLE,
            LAVAPIPE_THREADS_PER_DEMO.to_string(),
        )
        .env(ICED_TEST_BACKEND_VARIABLE, RECORDING_BACKEND)
        .output()
        .map_err(|spawn_error| spawn_error.to_string())?;
    if child_output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}\n{}",
            child_output.status,
            OsStr::from_bytes(&child_output.stderr).display()
        ))
    }
}

fn render_demos(demos: &[&Demo], jobs: NonZeroUsize, output: &Path) -> Result<(), Box<dyn Error>> {
    let executable = env::current_exe()?;
    let next_demo_position = AtomicUsize::new(0);
    let finished_demo_count = AtomicUsize::new(0);
    let run_started = Instant::now();
    let render_queued_demos = || {
        iter::from_fn(|| demos.get(next_demo_position.fetch_add(1, Ordering::Relaxed)))
            .filter_map(|demo| {
                let demo_started = Instant::now();
                let render_outcome = render_in_child_process(&executable, demo, output);
                let finished = finished_demo_count.fetch_add(1, Ordering::Relaxed) + 1;
                match render_outcome {
                    Ok(()) => {
                        eprintln!(
                            "[{finished}/{}] {} recorded in {:.1}s",
                            demos.len(),
                            demo.name,
                            seconds_since(demo_started)
                        );
                        None
                    }
                    Err(failure_report) => {
                        eprintln!(
                            "[{finished}/{}] {} failed: {failure_report}",
                            demos.len(),
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
        demos.len(),
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
    let selected_demos = select_demos(&command_line.demo_names)?;
    if command_line.render_in_this_process {
        selected_demos
            .iter()
            .for_each(|demo| render_single_demo(demo, &command_line.output));
        return Ok(());
    }
    render_demos(&selected_demos, command_line.jobs, &command_line.output)
}
