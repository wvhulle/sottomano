use std::{iter::successors, path::PathBuf};

use procfs::process::Process as Proc;
use sottomano_compositor_model::{
    compositor_window_facts::{WindowAppId, WindowPid, WindowTitle},
    compositor_window_state::CompositorWindowState,
};

use super::focus_units::FocusUnit;
use crate::desktop_windows_apps::{
    project_directory_labels::repository_root, terminal_emulator_windows::WindowProcesses,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusIdentity {
    pub window: u64,
    pub title: WindowTitle,
    pub app_id: String,
    pub probe_target: ProjectProbeTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectProbeTarget {
    Process(i32),
    Unreported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusProject {
    Repository(PathBuf),
    OutsideRepository,
}

impl FocusIdentity {
    #[must_use]
    pub fn focused_window_identity(
        compositor: &CompositorWindowState,
        processes: &WindowProcesses,
    ) -> Option<Self> {
        let window = compositor.focused_window()?;
        let WindowAppId::Reported(app_id) = &window.app_id else {
            return None;
        };
        Some(Self {
            window: window.id,
            title: window.title.clone(),
            app_id: app_id.clone(),
            probe_target: match processes.shown_in(window.id) {
                Some(shell) => ProjectProbeTarget::Process(shell),
                None => match window.pid {
                    WindowPid::Reported(pid) => ProjectProbeTarget::Process(pid),
                    WindowPid::Unreported => ProjectProbeTarget::Unreported,
                },
            },
        })
    }

    #[must_use]
    pub fn probed(self) -> Focused {
        Focused {
            window: self.window,
            project: match self.probe_target {
                ProjectProbeTarget::Process(pid) => probe_project(pid),
                ProjectProbeTarget::Unreported => FocusProject::OutsideRepository,
            },
            app_id: self.app_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Focused {
    pub window: u64,
    pub app_id: String,
    pub project: FocusProject,
}

impl Focused {
    pub(super) fn units(&self) -> Vec<FocusUnit> {
        let app_unit = FocusUnit::App(self.app_id.clone());
        match &self.project {
            FocusProject::Repository(root) => vec![app_unit, FocusUnit::Project(root.clone())],
            FocusProject::OutsideRepository => vec![app_unit],
        }
    }
}

fn probe_project(pid: i32) -> FocusProject {
    leaf_repository(pid).map_or(FocusProject::OutsideRepository, FocusProject::Repository)
}

fn leaf_repository(pid: i32) -> Option<PathBuf> {
    let leaf = successors(readable_process(pid), |process| {
        readable_process(only_child(process)?)
    })
    .last()?;
    let leaf_directory = leaf
        .cwd()
        .inspect_err(|cwd_failure| {
            tracing::debug!(
                "focus project: working directory of {} unreadable: {cwd_failure}",
                leaf.pid
            );
        })
        .ok()?;
    repository_root(&leaf_directory)
}

fn readable_process(pid: i32) -> Option<Proc> {
    Proc::new(pid)
        .inspect_err(|proc_failure| {
            tracing::debug!("focus project: process {pid} unreadable: {proc_failure}");
        })
        .ok()
}

fn only_child(process: &Proc) -> Option<i32> {
    let children = process
        .task_main_thread()
        .and_then(|main_thread| main_thread.children())
        .inspect_err(|children_failure| {
            tracing::debug!(
                "focus project: children of {} unreadable: {children_failure}",
                process.pid
            );
        })
        .ok()?;
    match children.as_slice() {
        [child] => i32::try_from(*child)
            .inspect_err(|range_failure| {
                tracing::warn!("focus project: child pid {child} out of range: {range_failure}");
            })
            .ok(),
        _ => None,
    }
}
