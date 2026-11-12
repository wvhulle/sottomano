pub mod ai_usage_spend;
pub mod apptainer_containers;
pub mod backing_service_availability;
pub mod battery_power_profiles;
pub mod blocking_background_work;
pub mod caldav_tasks_events;
pub mod cgroup_memory_limits;
pub mod ci_runner_tasks;
pub mod compositor_window_ipc;
pub mod cpu_memory_compute;
pub mod delivery_cadence_drivers;
pub mod desktop_notifications;
pub mod desktop_windows_apps;
pub mod disk_storage;
pub mod docker_container_engine;
pub mod flatpak_sandboxes;
pub mod gamepad_controller_input;
pub mod host_machine_identity;
pub mod incus_instances;
pub mod login_session_presence;
pub mod lxc_containers;
pub mod media_playback_audio;
pub mod microvm_guests;
pub mod network_connectivity;
pub mod non_empty_text;
pub mod nspawn_containers;
pub mod podman_container_engine;
pub mod qemu_virtual_machines;
pub mod restarting_counters;
pub mod screen_brightness_color;
pub mod secret_file_reads;
pub mod sqlite_state_store;
pub mod system_protocol_clients;
pub mod systemd_units_journal;
pub mod virtualbox_machines;
pub mod vmware_machines;
pub mod voice_dictation_input;
pub mod wallpaper_place_article;
pub mod waydroid_container;
pub mod weather;

use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use futures::Stream;

use crate::{
    delivery_cadence_drivers::DeliveryCadence,
    network_connectivity::internet_connectivity_gate::InternetUnreachable,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCaption {
    Uncaptioned,
    Captioned(&'static str),
}

pub trait SnapshotSource: Clone + Debug + Sized + Send + 'static {
    type Error: Debug + Display + Send + 'static;
    type StreamInputs: Clone + Hash + Send + Sync + 'static;

    const NAME: &'static str;
    const CAPTION: SourceCaption = SourceCaption::Uncaptioned;

    fn delivery_stream(
        key: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send;
}

#[derive(Debug)]
pub enum SnapshotDelivery<S: SnapshotSource> {
    SourceUnavailable,
    InternetUnreachable(InternetUnreachable),
    SnapshotDelivered {
        snapshot_fetch: Result<S, S::Error>,
        delivery_cadence: DeliveryCadence,
    },
    SnapshotUnchanged,
}

impl<S: SnapshotSource + PartialEq> PartialEq for SnapshotDelivery<S> {
    fn eq(&self, other: &Self) -> bool {
        match self {
            SnapshotDelivery::SourceUnavailable => {
                matches!(other, SnapshotDelivery::SourceUnavailable)
            }
            SnapshotDelivery::InternetUnreachable(reason) => matches!(
                other,
                SnapshotDelivery::InternetUnreachable(known_reason) if known_reason == reason
            ),
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(snapshot),
                delivery_cadence,
            } => matches!(
                other,
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(known_snapshot),
                    delivery_cadence: known_cadence,
                } if known_snapshot == snapshot && known_cadence == delivery_cadence
            ),
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Err(_),
                ..
            } => false,
            SnapshotDelivery::SnapshotUnchanged => {
                matches!(other, SnapshotDelivery::SnapshotUnchanged)
            }
        }
    }
}
