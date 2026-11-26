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
use sottomano_toml_config::Config;
use sottomano_widget_showcase::{
    WORKSPACE_ROOT,
    animation_recording::RecordingSession,
    demo_script::{DEMOS, Demo, UiPart},
    showcase_program::{ShowcaseProgram, booting_clock},
};

const LAVAPIPE_THREADS_VARIABLE: &str = "LP_NUM_THREADS";
const LAVAPIPE_THREADS_PER_DEMO: usize = 4;

#[derive(Debug, Parser)]
#[command(
    name = "sottomano-widget-showcase",
    about = "Render the tour recordings of sottomano into recordings/."
)]
struct CommandLine {
    #[arg(
        help = "Render only these demos: demo names, or UI parts such as hover-cards or toasts \
                that select every demo exercising them. --list shows both."
    )]
    selectors: Vec<String>,
    #[arg(
        long,
        help = "List the selected demos, or all of them, with the UI parts each exercises."
    )]
    list: bool,
    #[arg(
        long,
        conflicts_with = "selectors",
        help = "Open the demo fixtures in a window with the iced tester instead of recording."
    )]
    window: bool,
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
}

fn default_render_jobs() -> NonZeroUsize {
    let available_cores =
        thread::available_parallelism().expect("the number of available cores is queryable");
    NonZeroUsize::new(available_cores.get() / LAVAPIPE_THREADS_PER_DEMO)
        .unwrap_or(NonZeroUsize::MIN)
}

fn selected_by(demo: &Demo, selector: &str) -> bool {
    demo.name == selector
        || UiPart::named(selector).is_some_and(|ui_part| demo.parts.contains(&ui_part))
}

fn select_demos(selectors: &[String]) -> Result<Vec<&'static Demo>, Box<dyn Error>> {
    let selected: Vec<&Demo> = DEMOS
        .iter()
        .filter(|demo| {
            selectors.is_empty() || selectors.iter().any(|selector| selected_by(demo, selector))
        })
        .collect();
    if selected.is_empty() {
        let known_parts: Vec<String> = UiPart::ALL.iter().map(UiPart::to_string).collect();
        return Err(format!(
            "{} select no demo; the UI parts are {}",
            selectors.join(" "),
            known_parts.join(", ")
        )
        .into());
    }
    Ok(selected)
}

fn list_demos(demos: &[&Demo]) {
    demos.iter().for_each(|demo| {
        let part_names: Vec<String> = demo.parts.iter().map(UiPart::to_string).collect();
        println!("{}\t{}", demo.name, part_names.join(" "));
    });
}

fn render_single_demo(demo: &'static Demo, output: &Path) {
    let start = booting_clock();
    RecordingSession::open(start).record(demo, start, output);
}

fn render_in_child_process(executable: &Path, demo: &Demo, output: &Path) -> Result<(), String> {
    let child_output = Command::new(executable)
        .arg(demo.name)
        .arg("--output")
        .arg(output)
        .env(
            LAVAPIPE_THREADS_VARIABLE,
            LAVAPIPE_THREADS_PER_DEMO.to_string(),
        )
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
    if command_line.window {
        let program = ShowcaseProgram::interactive(&Config::default());
        return Ok(iced_winit::run(iced_tester::attach(program))?);
    }
    let selected_demos = select_demos(&command_line.selectors)?;
    if command_line.list {
        list_demos(&selected_demos);
        return Ok(());
    }
    match selected_demos.as_slice() {
        [single_demo] => {
            render_single_demo(single_demo, &command_line.output);
            Ok(())
        }
        several_demos => render_demos(several_demos, command_line.jobs, &command_line.output),
    }
}
