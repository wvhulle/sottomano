//! Native per-workspace launch-frequency tracking for quick-launch, replacing
//! the external `niri-window-tracker` daemon.
//!
//! Driven by the niri event stream odin-rs already owns: the first sighting of a
//! new window counts as a launch of its `app_id` on the window's *named*
//! workspace, stamped with the time. The snapshot seeded at connect (windows
//! already open) is never counted. Ranking is a recency-weighted frequency
//! ("frecency") so recent use outranks long-ago-heavy use.
//!
//! This is a pure in-memory model — no I/O. It ranks synchronously (nothing on
//! the render path), and persistence lives in
//! [`launch_store`](crate::data_sources::launch_store), which loads the model at
//! startup and writes back the [`LaunchDelta`] each counted launch yields.
//!
//! Unlike the daemon it replaces, this tracks only count and recency (no
//! per-window `cwd`), so relaunching always goes through the desktop entry.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use niri_ipc::Event;

/// A launch's weight halves every 14 days of age (odin's `HALFLIFE = 14day`).
const HALFLIFE_DAYS: f64 = 14.0;

#[derive(Debug, Clone)]
pub struct Record {
    pub count: u64,
    pub last_used: DateTime<Utc>,
}

/// The model: `{ workspace_name: { app_id: record } }`. Also the shape the store
/// loads into and reads from.
pub type Buckets = HashMap<String, HashMap<String, Record>>;

/// A counted launch to persist: the new absolute count and time for one
/// (workspace, app-id), so the store mirrors memory rather than re-incrementing.
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
    /// Workspace id → name, kept current from `WorkspacesChanged` so a launch
    /// can be attributed to its named workspace.
    workspace_names: HashMap<u64, String>,
    /// Window ids already sighted, so a launch is counted once and the initial
    /// snapshot is not counted at all.
    seen: HashSet<u64>,
    seeded: bool,
}

impl LaunchTracker {
    /// Install the model loaded from the store at startup, preserving the
    /// runtime state (workspace names, sightings) seeded from the event stream.
    pub fn load_from(&mut self, buckets: Buckets) {
        self.buckets = buckets;
    }

    /// Fold one niri event into the model. Returns the launch to persist when a
    /// new window was counted (the caller then persists it and re-ranks).
    pub fn observe(&mut self, event: &Event) -> Option<LaunchDelta> {
        match event {
            Event::WorkspacesChanged { workspaces } => {
                self.workspace_names = workspaces
                    .iter()
                    .filter_map(|ws| ws.name.clone().map(|name| (ws.id, name)))
                    .collect();
                None
            }
            // The full snapshot at connect: seed the sighting set without
            // counting anything.
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

    /// The ranked candidates for a workspace: its launched apps ordered by
    /// frecency (ties broken by recency, then app-id for a stable order),
    /// excluding the already-open and the unlaunchable, capped at `limit`.
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
        // Highest score first, then most recent, then app-id as a final
        // deterministic key so equal-scored entries never reshuffle between
        // re-ranks. NaN never occurs, so `partial_cmp` gives a total order.
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
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

/// Recency-weighted frequency: `count * 0.5^(age / halflife)`.
fn frecency(record: &Record, now: DateTime<Utc>) -> f64 {
    let age_days = (now - record.last_used).num_seconds() as f64 / 86_400.0;
    record.count as f64 * 0.5_f64.powf(age_days / HALFLIFE_DAYS)
}
