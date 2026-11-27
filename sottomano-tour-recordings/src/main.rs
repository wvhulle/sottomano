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

use clap::{Args, Parser, Subcommand};
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
const RENDER_IN_THIS_PROCESS_COMMAND: &str = "render-in-this-process";

#[derive(Debug, Parser)]
#[command(
    name = "record",
    about = "Render the tour recordings of sottomano into recordings/."
)]
struct CommandLine {
    #[command(subcommand)]
    recorder_command: RecorderCommand,
}

#[derive(Debug, Subcommand)]
enum RecorderCommand {
    #[command(about = "Print every demo with the scene it records.")]
    List,
    #[command(about = "Render demos, each in a child process of its own.")]
    Render {
        #[arg(help = "Render only these demos; all demos when none are named.")]
        demo_names: Vec<String>,
        #[arg(
            long,
            default_value_t = default_render_jobs(),
            help = "How many demos to render at the same time, by default one per four cores. \
                    Each demo buffers its frames, up to 2 GB."
        )]
        jobs: NonZeroUsize,
        #[command(flatten)]
        output_directory: OutputDirectory,
    },
    #[command(hide = true)]
    RenderInThisProcess {
        demo_name: String,
        #[command(flatten)]
        output_directory: OutputDirectory,
    },
}

#[derive(Debug, Args)]
struct OutputDirectory {
    #[arg(
        long = "output",
        default_value = WORKSPACE_ROOT,
        help = "Directory to write recordings/ into."
    )]
    path: PathBuf,
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
        .arg(RENDER_IN_THIS_PROCESS_COMMAND)
        .arg(demo.name)
        .arg("--output")
        .arg(output)
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

fn print_preview_commands(demos: &[&Demo], output_root: &Path) -> Result<(), Box<dyn Error>> {
    let output_root = output_root.canonicalize()?;
    demos.iter().for_each(|demo| {
        println!(
            "xdg-open {}",
            animation_recording::animation_path(&output_root, demo.name).display()
        );
    });
    Ok(())
}

fn seconds_since(started: Instant) -> f32 {
    Instant::now()
        .checked_duration_since(started)
        .expect("the monotonic clock does not run backwards")
        .as_secs_f32()
}

fn main() -> Result<(), Box<dyn Error>> {
    match CommandLine::parse().recorder_command {
        RecorderCommand::List => {
            DEMOS
                .iter()
                .for_each(|demo| println!("{:<32} {}", demo.name, demo.scene.name));
            Ok(())
        }
        RecorderCommand::Render {
            demo_names,
            jobs,
            output_directory,
        } => {
            let selected_demos = select_demos(&demo_names)?;
            render_demos(&selected_demos, jobs, &output_directory.path)?;
            print_preview_commands(&selected_demos, &output_directory.path)
        }
        RecorderCommand::RenderInThisProcess {
            demo_name,
            output_directory,
        } => {
            select_demos(&[demo_name])?
                .iter()
                .for_each(|demo| render_single_demo(demo, &output_directory.path));
            Ok(())
        }
    }
}
