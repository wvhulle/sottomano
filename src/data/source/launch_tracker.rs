use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use chrono::{DateTime, Utc};
use niri_ipc::Event;

const HALFLIFE_DAYS: f64 = 14.0;

#[derive(Debug, Clone)]
pub struct Record {
    pub count: u64,
    pub last_used: DateTime<Utc>,
}

pub type Buckets = HashMap<String, HashMap<String, Record>>;

#[derive(Debug, Clone)]
pub struct LaunchDelta {
    pub workspace: String,
    pub app_id: String,
    pub count: u64,
    pub last_used: DateTime<Utc>,
}

#[derive(Default)]
pub struct LaunchTracker {
    buckets: Buckets,
    workspace_names: HashMap<u64, String>,
    seen: HashSet<u64>,
    seeded: bool,
}

impl LaunchTracker {
    pub fn load_from(&mut self, buckets: Buckets) {
        self.buckets = buckets;
    }

    pub fn observe(&mut self, event: &Event) -> Option<LaunchDelta> {
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
                if !self.seeded || !self.seen.insert(window.id) {
                    return None;
                }
                let app_id = window.app_id.clone().filter(|id| !id.is_empty())?;
                let workspace = window
                    .workspace_id
                    .and_then(|id| self.workspace_names.get(&id).cloned())?;
                Some(self.record_launch(workspace, app_id))
            }
            Event::WindowClosed { id } => {
                self.seen.remove(id);
                None
            }
            _ => None,
        }
    }

    fn record_launch(&mut self, workspace: String, app_id: String) -> LaunchDelta {
        let now = Utc::now();
        let record = self
            .buckets
            .entry(workspace.clone())
            .or_default()
            .entry(app_id.clone())
            .or_insert(Record {
                count: 0,
                last_used: now,
            });
        record.count += 1;
        record.last_used = now;
        LaunchDelta {
            workspace,
            app_id,
            count: record.count,
            last_used: now,
        }
    }

    pub fn rank(
        &self,
        workspace: &str,
        exclude: &[String],
        limit: usize,
        now: DateTime<Utc>,
        is_launchable: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        if workspace.is_empty() {
            return Vec::new();
        }
        let Some(bucket) = self.buckets.get(workspace) else {
            return Vec::new();
        };
        let mut scored: Vec<(f64, DateTime<Utc>, &str)> = bucket
            .iter()
            .filter(|(app_id, _)| !exclude.iter().any(|e| e == *app_id))
            .filter(|(app_id, _)| is_launchable(app_id))
            .map(|(app_id, record)| (frecency(record, now), record.last_used, app_id.as_str()))
            .collect();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(Ordering::Equal)
                .then(b.1.cmp(&a.1))
                .then(a.2.cmp(b.2))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, _, app_id)| app_id.to_string())
            .collect()
    }
}

fn frecency(record: &Record, now: DateTime<Utc>) -> f64 {
    let age_days = (now - record.last_used).num_seconds() as f64 / 86_400.0;
    record.count as f64 * 0.5_f64.powf(age_days / HALFLIFE_DAYS)
}
