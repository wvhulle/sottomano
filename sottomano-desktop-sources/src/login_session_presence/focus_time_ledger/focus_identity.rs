use std::{iter::successors, path::PathBuf};

use procfs::process::Process as Proc;

use super::focus_units::FocusUnit;
use crate::{
    compositor_window_ipc::compositor_window_state::CompositorWindowState,
    desktop_windows_apps::{
        project_directory_labels::repository_root, terminal_emulator_windows::WindowProcesses,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusIdentity {
    pub window: u64,
    pub title: Option<String>,
    pub app_id: String,
    pub pid: Option<i32>,
    pub shell: Option<i32>,
}

impl FocusIdentity {
    #[must_use]
    pub fn of(compositor: &CompositorWindowState, processes: &WindowProcesses) -> Option<Self> {
        let window = compositor.focused_window()?;
        let app_id = window.app_id.clone().filter(|app_id| !app_id.is_empty())?;
        Some(Self {
            window: window.id,
            title: window.title.clone(),
            app_id,
            pid: window.pid,
            shell: processes.shown_in(window.id),
        })
    }

    pub fn probed(self) -> Focused {
        Focused {
            window: self.window,
            project: self.shell.or(self.pid).and_then(probe_project),
            app_id: self.app_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Focused {
    pub window: u64,
    pub app_id: String,
    pub project: Option<PathBuf>,
}

impl Focused {
    pub(super) fn units(&self) -> Vec<FocusUnit> {
        [Some(FocusUnit::App(self.app_id.clone()))]
            .into_iter()
            .chain([self.project.clone().map(FocusUnit::Project)])
            .flatten()
            .collect()
    }
}

fn probe_project(pid: i32) -> Option<PathBuf> {
    let leaf = successors(Proc::new(pid).ok(), |process| {
        Proc::new(only_child(process)?).ok()
    })
    .last()?;
    repository_root(&leaf.cwd().ok()?)
}

fn only_child(process: &Proc) -> Option<i32> {
    let children = process.task_main_thread().ok()?.children().ok()?;
    match children.as_slice() {
        [child] => i32::try_from(*child).ok(),
        _ => None,
    }
}
