use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use futures::{Stream, StreamExt, stream};
use serde::Deserialize;
use uom::si::{f64::Frequency, frequency::hertz};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::{dbus_desktop_buses::DesktopBuses, http_json_client},
    systemd_units_journal::{
        ScopedSystemdUnit, SystemdUnitOnBus, SystemdUnitScope, service_commands::ServiceCommand,
        unit_active_state::unit_running,
    },
};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const PROGRAM_NAME: &str = "llama-server";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "8080";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LlamaServerUnit {
    pub unit: ScopedSystemdUnit,
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LlamaServerOnBus {
    pub buses: DesktopBuses,
    pub server: LlamaServerUnit,
}

impl LlamaServerUnit {
    #[must_use]
    pub fn discovered(scope: SystemdUnitScope, command: &ServiceCommand) -> Option<Self> {
        (command.program_name() == Some(PROGRAM_NAME)).then(|| Self {
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

#[derive(Debug, Clone, PartialEq)]
pub struct LlamaModel {
    pub id: String,
    pub status: LlamaModelStatus,
    pub instance: Option<ModelInstance>,
    pub model_file: Option<PathBuf>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct LlamaSlot {
    pub id: u32,
    pub context: u32,
    pub processing: bool,
    pub task: Option<SlotTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlamaServer {
    pub models: Vec<LlamaModel>,
    pub slots: Vec<LlamaSlot>,
    pub generation_rate: Option<Frequency>,
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
        self.slots.iter().filter(|slot| slot.processing)
    }

    fn rated_since(self, previous: Option<&Self>) -> Self {
        let generation_rate = previous.and_then(|previous| {
            let elapsed = self
                .sampled_at
                .duration_since(previous.sampled_at)
                .as_secs_f64();
            let decoded: Vec<u32> = self
                .busy_slots()
                .filter_map(|slot| {
                    let task = slot.task?;
                    let before = previous
                        .slots
                        .iter()
                        .find(|earlier| earlier.id == slot.id)?
                        .task
                        .filter(|before| before.id == task.id)?;
                    Some(task.decoded_tokens.saturating_sub(before.decoded_tokens))
                })
                .collect();
            (elapsed > 0.0 && !decoded.is_empty())
                .then(|| Frequency::new::<hertz>(f64::from(decoded.iter().sum::<u32>()) / elapsed))
        });
        Self {
            generation_rate,
            ..self
        }
    }
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ListedModel>,
}

#[derive(Deserialize)]
struct ListedModel {
    id: String,
    status: ListedStatus,
}

#[derive(Deserialize)]
struct ListedStatus {
    value: LlamaModelStatus,
    #[serde(default)]
    args: Vec<String>,
}

impl ListedStatus {
    fn argument(&self, flag: &str) -> Option<&str> {
        let position = self.args.iter().position(|arg| arg == flag)?;
        self.args.get(position + 1).map(String::as_str)
    }

    fn instance(&self) -> Option<ModelInstance> {
        Some(ModelInstance {
            host: self.argument("--host")?.to_owned(),
            port: self
                .argument("--port")?
                .parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "llama server: invalid --port argument in {:?}: {parse_failure}",
                        self.args
                    );
                })
                .ok()?,
        })
    }
}

impl From<ListedModel> for LlamaModel {
    fn from(listed: ListedModel) -> Self {
        let instance = matches!(
            listed.status.value,
            LlamaModelStatus::Loaded | LlamaModelStatus::Sleeping
        )
        .then(|| listed.status.instance())
        .flatten();
        let model_file = listed.status.argument("--model").map(PathBuf::from);
        Self {
            id: listed.id,
            status: listed.status.value,
            instance,
            model_file,
        }
    }
}

#[derive(Deserialize)]
struct ListedSlot {
    id: u32,
    n_ctx: u32,
    is_processing: bool,
    id_task: Option<i64>,
    n_prompt_tokens: Option<u32>,
    #[serde(default)]
    next_token: Vec<NextToken>,
}

#[derive(Deserialize)]
struct NextToken {
    n_decoded: u32,
}

impl From<ListedSlot> for LlamaSlot {
    fn from(listed: ListedSlot) -> Self {
        let task = listed
            .id_task
            .zip(listed.n_prompt_tokens)
            .and_then(|(id, prompt_tokens)| {
                Some(SlotTask {
                    id,
                    prompt_tokens,
                    decoded_tokens: listed.next_token.first()?.n_decoded,
                })
            });
        Self {
            id: listed.id,
            context: listed.n_ctx,
            processing: listed.is_processing,
            task,
        }
    }
}

async fn fetch_server(endpoint: &str) -> Result<LlamaServer, http_json_client::Error> {
    let listed: ModelList =
        http_json_client::get_json(&format!("{endpoint}/v1/models"), &[]).await?;
    let models: Vec<LlamaModel> = listed.data.into_iter().map(LlamaModel::from).collect();
    let serving = models
        .iter()
        .filter(|model| model.status == LlamaModelStatus::Loaded)
        .find_map(|model| model.instance.as_ref());
    let slots = match serving {
        Some(instance) => {
            http_json_client::get_json::<Vec<ListedSlot>>(&instance.url("slots"), &[])
                .await?
                .into_iter()
                .map(LlamaSlot::from)
                .collect()
        }
        None => Vec::new(),
    };
    Ok(LlamaServer {
        models,
        slots,
        generation_rate: None,
        sampled_at: Instant::now(),
    })
}

impl SnapshotSource for LlamaServer {
    type Error = http_json_client::Error;
    type StreamInputs = LlamaServerOnBus;

    const NAME: &'static str = "Llama server";

    fn delivery_stream(
        watch: LlamaServerOnBus,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let LlamaServerOnBus { buses, server } = watch;
        let running = unit_running(SystemdUnitOnBus {
            buses,
            unit: server.unit.clone(),
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
                        SnapshotDelivery::InternetUnreachable(reason) => {
                            SnapshotDelivery::InternetUnreachable(reason)
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_models_carry_status_and_child_instance() {
        let listed: ModelList = serde_json::from_str(
            r#"{"data":[
                {"id":"Qwen3.6","status":{"value":"unloaded","args":["llama-server","--host","127.0.0.1","--port","0"]}},
                {"id":"Qwen3.8","status":{"value":"loaded","args":["llama-server","--host","127.0.0.1","--port","57883","--ctx-size","32768","--model","/models/Qwen3.8-27B.gguf"]}}
            ]}"#,
        )
        .unwrap();
        let models: Vec<LlamaModel> = listed.data.into_iter().map(LlamaModel::from).collect();
        assert_eq!(models[0].status, LlamaModelStatus::Unloaded);
        assert_eq!(models[0].instance, None);
        assert_eq!(models[0].model_file, None);
        assert_eq!(
            models[1].model_file,
            Some(PathBuf::from("/models/Qwen3.8-27B.gguf"))
        );
        assert_eq!(
            models[1].instance,
            Some(ModelInstance {
                host: "127.0.0.1".to_owned(),
                port: 57883,
            })
        );
    }

    #[test]
    fn slots_without_a_task_have_no_token_counts() {
        let listed: Vec<ListedSlot> = serde_json::from_str(
            r#"[
                {"id":0,"n_ctx":32768,"speculative":false,"is_processing":false},
                {"id":3,"n_ctx":32768,"is_processing":true,"id_task":182,"n_prompt_tokens":11745,
                 "next_token":[{"has_next_token":true,"n_remain":-1,"n_decoded":129}]}
            ]"#,
        )
        .unwrap();
        let slots: Vec<LlamaSlot> = listed.into_iter().map(LlamaSlot::from).collect();
        assert_eq!(slots[0].task, None);
        assert_eq!(slots[1].task.map(|task| task.tokens()), Some(11874));
    }
}
