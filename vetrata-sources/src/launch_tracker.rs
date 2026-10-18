use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use chrono::{DateTime, Utc};
use niri_ipc::{Event, Window};

const HALFLIFE_DAYS: f64 = 14.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    pub workspace: String,
    pub app_id: String,
    pub launched_at: DateTime<Utc>,
}

#[derive(Default)]
pub struct LaunchTracker {
    history: Vec<Launch>,
    workspace_names: HashMap<u64, String>,
    seen: HashSet<u64>,
    seeded: bool,
}

impl LaunchTracker {
    pub fn load_from(&mut self, mut stored: Vec<Launch>) {
        self.history.retain(|launch| !stored.contains(launch));
        stored.append(&mut self.history);
        self.history = stored;
    }

    pub fn observe(&mut self, event: &Event) -> Option<Launch> {
        match event {
            Event::WorkspacesChanged { workspaces } => {
                self.workspace_names = workspaces
                    .iter()
                    .filter_map(|ws| ws.name.clone().map(|name| (ws.id, name)))
                    .collect();
                None
            }
            Event::WindowsChanged { windows } => {
                self.seen = windows.iter().map(|window| window.id).collect();
                self.seeded = true;
                None
            }
            Event::WindowOpenedOrChanged { window } => {
                let launch = self.first_launch(window)?;
                self.history.push(launch.clone());
                Some(launch)
            }
            Event::WindowClosed { id } => {
                self.seen.remove(id);
                None
            }
            _ => None,
        }
    }

    fn first_launch(&mut self, window: &Window) -> Option<Launch> {
        if !self.seeded || self.seen.contains(&window.id) {
            return None;
        }
        let app_id = window.app_id.clone().filter(|id| !id.is_empty())?;
        let workspace = self.workspace_names.get(&window.workspace_id?)?.clone();
        self.seen.insert(window.id);
        Some(Launch {
            workspace,
            app_id,
            launched_at: Utc::now(),
        })
    }

    pub fn rank(
        &self,
        workspace: &str,
        exclude: &[String],
        now: DateTime<Utc>,
        is_launchable: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        let frecencies = self
            .history
            .iter()
            .filter(|launch| launch.workspace == workspace)
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
            .filter(|(app_id, _)| !exclude.iter().any(|e| e == app_id))
            .filter(|(app_id, _)| is_launchable(app_id))
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
    use niri_ipc::{Event, Window, WindowLayout, Workspace};

    use super::{Launch, LaunchTracker};

    fn workspace(id: u64, name: &str) -> Workspace {
        Workspace {
            id,
            idx: 1,
            name: Some(name.to_owned()),
            output: Some("DP-1".to_owned()),
            is_urgent: false,
            is_active: true,
            is_focused: true,
            active_window_id: None,
        }
    }

    fn window(id: u64, app_id: Option<&str>) -> Window {
        Window {
            id,
            title: None,
            app_id: app_id.map(str::to_owned),
            pid: None,
            workspace_id: Some(1),
            is_focused: false,
            is_floating: false,
            is_urgent: false,
            layout: WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }

    fn seeded_tracker() -> LaunchTracker {
        let mut tracker = LaunchTracker::default();
        tracker.observe(&Event::WorkspacesChanged {
            workspaces: vec![workspace(1, "web")],
        });
        tracker.observe(&Event::WindowsChanged {
            windows: Vec::new(),
        });
        tracker
    }

    fn opened(tracker: &mut LaunchTracker, window: Window) -> Option<Launch> {
        tracker.observe(&Event::WindowOpenedOrChanged { window })
    }

    fn launch(app_id: &str, days_ago: i64) -> Launch {
        Launch {
            workspace: "web".to_owned(),
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
    fn launches_before_loading_survive_without_doubling() {
        let mut tracker = seeded_tracker();
        let early = opened(&mut tracker, window(7, Some("code"))).unwrap();
        let persisted = opened(&mut tracker, window(8, Some("firefox"))).unwrap();

        tracker.load_from(vec![launch("foot", 3), persisted]);

        assert_eq!(tracker.history.len(), 3);
        assert!(tracker.history.contains(&early));
    }

    #[test]
    fn rank_prefers_frequent_recent_apps_and_skips_open_ones() {
        let mut tracker = LaunchTracker::default();
        tracker.load_from(vec![
            launch("foot", 0),
            launch("foot", 1),
            launch("firefox", 0),
            launch("mpv", 40),
            launch("code", 0),
        ]);

        let ranked = tracker.rank("web", &["code".to_owned()], Utc::now(), |_| true);

        assert_eq!(ranked, ["foot", "firefox", "mpv"]);
        assert!(tracker.rank("mail", &[], Utc::now(), |_| true).is_empty());
    }
}
