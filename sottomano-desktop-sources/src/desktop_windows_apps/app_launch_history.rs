use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use chrono::{DateTime, Utc};

use crate::desktop_windows_apps::compositor_window_state::{
    CompositorWindow, CompositorWindowState, CompositorWorkspace,
};

const HALFLIFE_DAYS: f64 = 14.0;

const UNNAMED_WORKSPACE_STORED_NAME: &str = "";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchWorkspace {
    Named(String),
    Unnamed,
}

impl LaunchWorkspace {
    #[must_use]
    pub fn stored_name(&self) -> &str {
        match self {
            LaunchWorkspace::Named(name) => name,
            LaunchWorkspace::Unnamed => UNNAMED_WORKSPACE_STORED_NAME,
        }
    }

    #[must_use]
    pub fn from_stored_name(stored_name: String) -> Self {
        if stored_name == UNNAMED_WORKSPACE_STORED_NAME {
            LaunchWorkspace::Unnamed
        } else {
            LaunchWorkspace::Named(stored_name)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    pub workspace: LaunchWorkspace,
    pub app_id: String,
    pub launched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchScope<'workspace> {
    NamedWorkspace(&'workspace str),
    EveryWorkspace,
}

impl LaunchScope<'_> {
    fn covers(self, launch: &Launch) -> bool {
        match self {
            LaunchScope::NamedWorkspace(wanted_name) => matches!(
                &launch.workspace,
                LaunchWorkspace::Named(workspace_name) if workspace_name == wanted_name
            ),
            LaunchScope::EveryWorkspace => true,
        }
    }
}

#[derive(Default)]
pub struct LaunchTracker {
    history: Vec<Launch>,
    seen: HashSet<u64>,
    seeded: bool,
}

impl LaunchTracker {
    pub fn load_from(&mut self, mut stored: Vec<Launch>) {
        self.history.retain(|launch| !stored.contains(launch));
        stored.append(&mut self.history);
        self.history = stored;
    }

    pub fn observe(&mut self, state: &CompositorWindowState) -> Vec<Launch> {
        if !self.seeded {
            self.seen = state.windows.keys().copied().collect();
            self.seeded = true;
            return Vec::new();
        }
        self.seen.retain(|id| state.windows.contains_key(id));
        let launches: Vec<Launch> = state
            .windows
            .values()
            .filter_map(|window| self.first_launch(window, &state.workspaces))
            .collect();
        self.history.extend(launches.iter().cloned());
        launches
    }

    fn first_launch(
        &mut self,
        window: &CompositorWindow,
        workspaces: &HashMap<u64, CompositorWorkspace>,
    ) -> Option<Launch> {
        if self.seen.contains(&window.id) {
            return None;
        }
        let app_id = window.app_id.clone().filter(|id| !id.is_empty())?;
        let workspace = workspaces
            .get(&window.workspace_id?)?
            .name
            .clone()
            .map_or(LaunchWorkspace::Unnamed, LaunchWorkspace::Named);
        self.seen.insert(window.id);
        Some(Launch {
            workspace,
            app_id,
            launched_at: Utc::now(),
        })
    }

    pub fn rank(&self, scope: LaunchScope<'_>, now: DateTime<Utc>) -> Vec<String> {
        let frecencies = self
            .history
            .iter()
            .filter(|launch| scope.covers(launch))
            .fold(
                HashMap::<&str, Frecency>::new(),
                |mut frecencies, launch| {
                    frecencies
                        .entry(launch.app_id.as_str())
                        .or_default()
                        .add(launch.launched_at, now);
                    frecencies
                },
            );
        let mut ranked: Vec<RankedApp> = frecencies
            .into_iter()
            .map(|(app_id, frecency)| RankedApp { app_id, frecency })
            .collect();
        ranked.sort_by(RankedApp::cmp_rank);
        ranked
            .into_iter()
            .map(|app| app.app_id.to_string())
            .collect()
    }
}

#[derive(Default)]
struct Frecency {
    weight: f64,
    last_launched: DateTime<Utc>,
}

impl Frecency {
    fn add(&mut self, launched_at: DateTime<Utc>, now: DateTime<Utc>) {
        let age_days = (now - launched_at).num_seconds() as f64 / 86_400.0;
        self.weight += 0.5_f64.powf(age_days / HALFLIFE_DAYS);
        self.last_launched = self.last_launched.max(launched_at);
    }
}

struct RankedApp<'a> {
    app_id: &'a str,
    frecency: Frecency,
}

impl RankedApp<'_> {
    fn cmp_rank(&self, other: &Self) -> Ordering {
        other
            .frecency
            .weight
            .total_cmp(&self.frecency.weight)
            .then(
                other
                    .frecency
                    .last_launched
                    .cmp(&self.frecency.last_launched),
            )
            .then(self.app_id.cmp(other.app_id))
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{Launch, LaunchScope, LaunchTracker, LaunchWorkspace};
    use crate::desktop_windows_apps::{
        compositor_test_windows::{unnamed_compositor_workspace, unplaced_compositor_window},
        compositor_window_state::{CompositorWindow, CompositorWindowState, CompositorWorkspace},
    };

    fn workspace(id: u64, name: &str) -> CompositorWorkspace {
        CompositorWorkspace {
            name: Some(name.to_owned()),
            output: Some("DP-1".to_owned()),
            ..unnamed_compositor_workspace(id)
        }
    }

    fn window(id: u64, app_id: Option<&str>) -> CompositorWindow {
        CompositorWindow {
            app_id: app_id.map(str::to_owned),
            ..unplaced_compositor_window(id)
        }
    }

    fn seeded_tracker() -> LaunchTracker {
        let mut tracker = LaunchTracker::default();
        tracker.observe(&CompositorWindowState::default());
        tracker
    }

    fn opened_among(
        tracker: &mut LaunchTracker,
        workspaces: Vec<CompositorWorkspace>,
        window: CompositorWindow,
    ) -> Option<Launch> {
        tracker
            .observe(&CompositorWindowState::from_lists(workspaces, [window]))
            .pop()
    }

    fn opened(tracker: &mut LaunchTracker, window: CompositorWindow) -> Option<Launch> {
        opened_among(tracker, vec![workspace(1, "web")], window)
    }

    fn launch(app_id: &str, days_ago: i64) -> Launch {
        Launch {
            workspace: LaunchWorkspace::Named("web".to_owned()),
            app_id: app_id.to_owned(),
            launched_at: Utc::now() - Duration::days(days_ago),
        }
    }

    #[test]
    fn a_window_that_gains_its_app_id_later_is_recorded_once() {
        let mut tracker = seeded_tracker();

        assert!(opened(&mut tracker, window(7, None)).is_none());
        assert!(opened(&mut tracker, window(7, Some("code"))).is_some());
        assert!(opened(&mut tracker, window(7, Some("code"))).is_none());
    }

    #[test]
    fn windows_open_before_the_first_state_are_not_launches() {
        let mut tracker = LaunchTracker::default();
        let initial =
            CompositorWindowState::from_lists([workspace(1, "web")], [window(7, Some("code"))]);

        assert!(tracker.observe(&initial).is_empty());
        assert!(tracker.observe(&initial).is_empty());
    }

    #[test]
    fn launches_before_loading_survive_without_doubling() {
        let mut tracker = seeded_tracker();
        let early = opened(&mut tracker, window(7, Some("code"))).unwrap();
        let persisted = opened(&mut tracker, window(8, Some("firefox"))).unwrap();

        tracker.load_from(vec![launch("foot", 3), persisted]);

        assert_eq!(tracker.history.len(), 3);
        assert!(tracker.history.contains(&early));
    }

    #[test]
    fn a_launch_on_an_unnamed_workspace_counts_only_for_every_workspace() {
        let mut tracker = seeded_tracker();
        let unnamed = CompositorWorkspace {
            name: None,
            ..workspace(1, "ignored")
        };

        let recorded = opened_among(&mut tracker, vec![unnamed], window(7, Some("foot"))).unwrap();

        assert_eq!(recorded.workspace, LaunchWorkspace::Unnamed);
        assert_eq!(
            tracker.rank(LaunchScope::EveryWorkspace, Utc::now()),
            ["foot"]
        );
        assert!(
            tracker
                .rank(LaunchScope::NamedWorkspace(""), Utc::now())
                .is_empty()
        );
    }

    #[test]
    fn a_window_waits_until_its_workspace_is_known() {
        let mut tracker = seeded_tracker();
        assert!(opened_among(&mut tracker, Vec::new(), window(7, Some("foot"))).is_none());

        let recorded = opened(&mut tracker, window(7, Some("foot"))).unwrap();
        assert_eq!(recorded.workspace, LaunchWorkspace::Named("web".to_owned()));
    }

    #[test]
    fn stored_workspace_names_round_trip() {
        for launch_workspace in [
            LaunchWorkspace::Unnamed,
            LaunchWorkspace::Named("web".to_owned()),
        ] {
            assert_eq!(
                LaunchWorkspace::from_stored_name(launch_workspace.stored_name().to_owned()),
                launch_workspace
            );
        }
    }

    #[test]
    fn rank_prefers_frequent_recent_apps_within_its_workspace() {
        let mut tracker = LaunchTracker::default();
        tracker.load_from(vec![
            launch("foot", 0),
            launch("foot", 1),
            launch("firefox", 0),
            launch("mpv", 40),
        ]);

        let ranked = tracker.rank(LaunchScope::NamedWorkspace("web"), Utc::now());

        assert_eq!(ranked, ["foot", "firefox", "mpv"]);
        assert!(
            tracker
                .rank(LaunchScope::NamedWorkspace("mail"), Utc::now())
                .is_empty()
        );
    }

    #[test]
    fn every_workspace_scope_sums_launches_across_workspaces() {
        let mut tracker = LaunchTracker::default();
        let launch_on_mail = |app_id: &str| Launch {
            workspace: LaunchWorkspace::Named("mail".to_owned()),
            ..launch(app_id, 0)
        };
        tracker.load_from(vec![
            launch("firefox", 0),
            launch("foot", 0),
            launch_on_mail("foot"),
        ]);

        assert_eq!(
            tracker.rank(LaunchScope::EveryWorkspace, Utc::now()),
            ["foot", "firefox"]
        );
    }
}
