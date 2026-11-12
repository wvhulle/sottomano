use procfs::process::{Process as Proc, Stat};
use tokio::task::spawn_blocking;

use crate::{
    desktop_windows_apps::{
        desktop_entry_app_icons::DesktopEntries,
        process_tree_inspection::{binary_name, launch},
    },
    non_empty_text::non_empty,
};

pub struct InhibitorAppNames {
    desktop_entries: DesktopEntries,
}

impl InhibitorAppNames {
    pub async fn load() -> Self {
        Self {
            desktop_entries: spawn_blocking(DesktopEntries::load).await.unwrap_or_else(
                |load_failure| {
                    tracing::warn!(
                        "inhibitor app names: desktop entries failed to load: {load_failure}"
                    );
                    DesktopEntries::empty()
                },
            ),
        }
    }

    #[must_use]
    pub fn of_process(&self, pid: u32) -> Option<String> {
        let process = readable_process(pid)?;
        self.scope_app_name(process.pid, &process)
            .or_else(|| own_process_name(&process))
    }

    #[must_use]
    pub fn of_lock_holder(&self, stated_name: &str, pid: u32) -> String {
        match stated_name_kind(stated_name, pid) {
            StatedNameKind::RuntimeOnly => self
                .of_process(pid)
                .unwrap_or_else(|| stated_name.to_owned()),
            StatedNameKind::AppName => stated_name.to_owned(),
        }
    }

    fn scope_app_name(&self, process_id: i32, process: &Proc) -> Option<String> {
        let own_scope = launch(process_id)?.scope?;
        let parent_id = logged_process_stat(process)?.ppid;
        let parent_scope = launch(parent_id).and_then(|parent| parent.scope);
        if parent_scope.as_deref() == Some(own_scope.as_str()) {
            return None;
        }
        let app_id = app_id_of_cgroup_leaf(&own_scope)?;
        Some(self.display_name(&app_id))
    }

    fn display_name(&self, app_id: &str) -> String {
        let without_launcher = app_id.split_once('-').map(|(_, rest)| rest);
        [Some(app_id), without_launcher]
            .into_iter()
            .flatten()
            .find_map(|candidate| self.desktop_entries.launchable_app(candidate))
            .map_or_else(|| app_id.to_owned(), |app| app.name)
    }
}

fn own_process_name(process: &Proc) -> Option<String> {
    binary_name(process).or_else(|| non_empty(logged_process_stat(process)?.comm))
}

fn readable_process(pid: u32) -> Option<Proc> {
    let process_id = i32::try_from(pid)
        .inspect_err(|range_failure| {
            tracing::warn!("inhibitor app names: pid {pid} out of range: {range_failure}");
        })
        .ok()?;
    Proc::new(process_id)
        .inspect_err(|proc_failure| {
            tracing::debug!("inhibitor app names: process {pid} unreadable: {proc_failure}");
        })
        .ok()
}

fn logged_process_stat(process: &Proc) -> Option<Stat> {
    process
        .stat()
        .inspect_err(|stat_failure| {
            tracing::debug!(
                "inhibitor app names: process {} stat unreadable: {stat_failure}",
                process.pid
            );
        })
        .ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatedNameKind {
    RuntimeOnly,
    AppName,
}

fn stated_name_kind(stated_name: &str, pid: u32) -> StatedNameKind {
    let runtime_only = stated_name.is_empty()
        || readable_process(pid).is_some_and(|process| {
            let comm = logged_process_stat(&process).map(|stat| stat.comm);
            [binary_name(&process), comm]
                .into_iter()
                .flatten()
                .any(|process_name| process_name.eq_ignore_ascii_case(stated_name))
        });
    if runtime_only {
        StatedNameKind::RuntimeOnly
    } else {
        StatedNameKind::AppName
    }
}

fn app_id_of_cgroup_leaf(leaf: &str) -> Option<String> {
    let unescaped = |name: &str| non_empty(name.replace("\\x2d", "-"));
    if let Some(scope_name) = leaf.strip_suffix(".scope") {
        let app_scope = scope_name.strip_prefix("app-")?;
        return unescaped(without_trailing_number(app_scope));
    }
    let service_name = leaf.strip_suffix(".service")?;
    let without_instance = service_name
        .split_once('@')
        .map_or(service_name, |(name, _)| name);
    unescaped(
        without_instance
            .strip_prefix("app-")
            .unwrap_or(without_instance),
    )
}

fn without_trailing_number(name: &str) -> &str {
    match name.rsplit_once('-') {
        Some((before, number)) if number.bytes().all(|byte| byte.is_ascii_digit()) => before,
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::app_id_of_cgroup_leaf;

    #[test]
    fn an_app_scope_names_its_app_without_the_random_number() {
        assert_eq!(
            app_id_of_cgroup_leaf("app-bitwarden-6370.scope").as_deref(),
            Some("bitwarden")
        );
    }

    #[test]
    fn an_app_service_names_its_app_without_the_instance() {
        assert_eq!(
            app_id_of_cgroup_leaf("app-foo@1234.service").as_deref(),
            Some("foo")
        );
    }

    #[test]
    fn a_plain_user_service_names_itself() {
        assert_eq!(
            app_id_of_cgroup_leaf("polkit-agent.service").as_deref(),
            Some("polkit-agent")
        );
    }

    #[test]
    fn escaped_dashes_are_restored() {
        assert_eq!(
            app_id_of_cgroup_leaf("app-foo\\x2dbar-12.scope").as_deref(),
            Some("foo-bar")
        );
    }

    #[test]
    fn session_and_manager_scopes_name_no_app() {
        assert_eq!(app_id_of_cgroup_leaf("init.scope"), None);
        assert_eq!(app_id_of_cgroup_leaf("session-2.scope"), None);
    }
}
