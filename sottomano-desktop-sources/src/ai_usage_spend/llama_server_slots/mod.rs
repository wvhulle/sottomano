mod llama_server_listings;
#[cfg(test)]
mod llama_slot_parsing_tests;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use futures::{Stream, StreamExt, stream};
use serde::Deserialize;
use uom::si::{f64::Frequency, frequency::hertz};

use self::llama_server_listings::fetch_server;
use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::{dbus_desktop_buses::OnBus, http_json_client},
    systemd_units_journal::{
        ScopedSystemdUnit, SystemdUnitScope, service_commands::ServiceCommand,
        unit_active_state::unit_running,
    },
};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
pub(crate) const LLAMA_SERVER_PROGRAM: &str = "llama-server";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "8080";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LlamaServerUnit {
    pub unit: ScopedSystemdUnit,
    pub endpoint: String,
}

impl LlamaServerUnit {
    #[must_use]
    pub fn discovered(scope: SystemdUnitScope, command: &ServiceCommand) -> Option<Self> {
        (command.program_name() == Some(LLAMA_SERVER_PROGRAM)).then(|| Self {
            unit: ScopedSystemdUnit {
                scope,
                unit_name: command.unit.clone(),
            },
            endpoint: format!(
                "http://{}:{}",
                command.argument("--host").unwrap_or(DEFAULT_HOST),
                command.argument("--port").unwrap_or(DEFAULT_PORT),
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlamaModelStatus {
    Downloading,
    Downloaded,
    Unloaded,
    Loading,
    Loaded,
    Sleeping,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelInstance {
    host: String,
    port: u16,
}

impl ModelInstance {
    fn url(&self, path: &str) -> String {
        format!("http://{}:{}/{path}", self.host, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildInstance {
    Listening(ModelInstance),
    NotListening,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelFileArgument {
    Given(PathBuf),
    NotGiven,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlamaModel {
    pub id: String,
    pub status: LlamaModelStatus,
    pub instance: ChildInstance,
    pub model_file: ModelFileArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotTask {
    id: i64,
    pub prompt_tokens: u32,
    pub decoded_tokens: u32,
}

impl SlotTask {
    #[must_use]
    pub fn tokens(&self) -> u32 {
        self.prompt_tokens + self.decoded_tokens
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotWork {
    Processing,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAssignment {
    Assigned(SlotTask),
    Unassigned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlamaSlot {
    pub id: u32,
    pub context: u32,
    pub work: SlotWork,
    pub task: SlotAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecodeRate {
    Measured(Frequency),
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlamaServer {
    pub models: Vec<LlamaModel>,
    pub slots: Vec<LlamaSlot>,
    pub generation_rate: DecodeRate,
    sampled_at: Instant,
}

impl LlamaServer {
    #[must_use]
    pub fn resident_model(&self) -> Option<&LlamaModel> {
        self.models.iter().find(|model| {
            matches!(
                model.status,
                LlamaModelStatus::Loading | LlamaModelStatus::Loaded | LlamaModelStatus::Sleeping
            )
        })
    }

    pub fn busy_slots(&self) -> impl Iterator<Item = &LlamaSlot> {
        self.slots
            .iter()
            .filter(|slot| slot.work == SlotWork::Processing)
    }

    fn rated_since(self, previous: Option<&Self>) -> Self {
        let measured_rate = previous.and_then(|previous| {
            let elapsed = self
                .sampled_at
                .checked_duration_since(previous.sampled_at)
                .expect("the previous slot sample was taken earlier")
                .as_secs_f64();
            let decoded: Vec<u32> = self
                .busy_slots()
                .filter_map(|slot| {
                    let SlotAssignment::Assigned(task) = slot.task else {
                        return None;
                    };
                    let earlier_slot = previous
                        .slots
                        .iter()
                        .find(|earlier| earlier.id == slot.id)?;
                    let SlotAssignment::Assigned(before) = earlier_slot.task else {
                        return None;
                    };
                    if before.id != task.id {
                        return None;
                    }
                    let Some(decoded_since) =
                        task.decoded_tokens.checked_sub(before.decoded_tokens)
                    else {
                        tracing::debug!(
                            "llama slots: task {} in slot {} decoded fewer tokens than before",
                            task.id,
                            slot.id
                        );
                        return None;
                    };
                    Some(decoded_since)
                })
                .collect();
            (elapsed > 0.0 && !decoded.is_empty())
                .then(|| Frequency::new::<hertz>(f64::from(decoded.iter().sum::<u32>()) / elapsed))
        });
        Self {
            generation_rate: measured_rate.map_or(DecodeRate::Unmeasured, DecodeRate::Measured),
            ..self
        }
    }
}

impl SnapshotSource for LlamaServer {
    type Error = http_json_client::Error;
    type StreamInputs = OnBus<LlamaServerUnit>;

    const NAME: &'static str = "Llama server";

    fn delivery_stream(
        watch: OnBus<LlamaServerUnit>,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let OnBus {
            buses,
            inputs: server,
        } = watch;
        let running = unit_running(OnBus {
            buses,
            inputs: server.unit.clone(),
        });
        running.while_available(
            move || {
                let endpoint = server.endpoint.clone();
                let mut previous = None;
                PeriodicPolling::every(POLL_INTERVAL)
                    .poll_deliveries(move || {
                        let endpoint = endpoint.clone();
                        async move { fetch_server(&endpoint).await }
                    })
                    .map(move |delivery| match delivery {
                        SnapshotDelivery::SnapshotDelivered {
                            snapshot_fetch: Ok(server),
                            delivery_cadence,
                        } => {
                            let rated_server = server.rated_since(previous.as_ref());
                            previous = Some(rated_server.clone());
                            SnapshotDelivery::SnapshotDelivered {
                                snapshot_fetch: Ok(rated_server),
                                delivery_cadence,
                            }
                        }
                        SnapshotDelivery::SnapshotDelivered {
                            snapshot_fetch: Err(failure),
                            delivery_cadence,
                        } => SnapshotDelivery::SnapshotDelivered {
                            snapshot_fetch: Err(failure),
                            delivery_cadence,
                        },
                        SnapshotDelivery::SourceUnavailable => SnapshotDelivery::SourceUnavailable,
                        SnapshotDelivery::SnapshotUnchanged => SnapshotDelivery::SnapshotUnchanged,
                    })
            },
            || SnapshotDelivery::SourceUnavailable,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Modalities {
    pub vision: bool,
    pub audio: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlamaModelProps {
    #[serde(rename = "model_path")]
    pub path: PathBuf,
    #[serde(rename = "model_ftype")]
    pub quantization: String,
    pub total_slots: u32,
    #[serde(rename = "build_info")]
    pub build: String,
    pub modalities: Modalities,
}

impl SnapshotSource for LlamaModelProps {
    type Error = http_json_client::Error;
    type StreamInputs = ModelInstance;

    const NAME: &'static str = "Model";

    fn delivery_stream(
        instance: ModelInstance,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(async move {
            SnapshotDelivery::single_read(
                http_json_client::get_json(&instance.url("props"), &[]).await,
            )
        })
    }
}
