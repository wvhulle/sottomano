use std::{future::ready, marker::PhantomData, path::PathBuf, time::Instant};

use iced::futures::{Stream, StreamExt, stream};
use serde::Deserialize;
use uom::si::{f64::Frequency, frequency::hertz};

use crate::data::source::{
    Delivery, LiveSource, Source, http,
    journal::unit_log_appends,
    present_or_absent,
    systemd_manager::{UnitScope, service_commands::ServiceCommand, unit_activity::unit_running},
};

const FETCH_TIMEOUT_SECS: u64 = 3;
const PROGRAM_NAME: &str = "llama-server";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "8080";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LlamaServerUnit<S: UnitScope> {
    pub unit: String,
    pub endpoint: String,
    scope: PhantomData<S>,
}

impl<S: UnitScope> LlamaServerUnit<S> {
    pub fn discovered(command: &ServiceCommand) -> Option<Self> {
        command.runs(PROGRAM_NAME).then(|| Self {
            unit: command.unit.clone(),
            endpoint: format!(
                "http://{}:{}",
                command.argument("--host").unwrap_or(DEFAULT_HOST),
                command.argument("--port").unwrap_or(DEFAULT_PORT),
            ),
            scope: PhantomData,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotTask {
    id: i64,
    pub prompt_tokens: u32,
    pub decoded_tokens: u32,
}

impl SlotTask {
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
pub struct LlamaServer<S: UnitScope> {
    pub models: Vec<LlamaModel>,
    pub slots: Vec<LlamaSlot>,
    pub generation_rate: Option<Frequency>,
    sampled_at: Instant,
    scope: PhantomData<S>,
}

impl<S: UnitScope> LlamaServer<S> {
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
            port: self.argument("--port")?.parse().ok()?,
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
        Self {
            id: listed.id,
            status: listed.status.value,
            instance,
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

async fn fetch_server<S: UnitScope>(endpoint: &str) -> Result<LlamaServer<S>, http::Error> {
    let listed: ModelList =
        http::get_json(&format!("{endpoint}/v1/models"), &[], FETCH_TIMEOUT_SECS).await?;
    let models: Vec<LlamaModel> = listed.data.into_iter().map(LlamaModel::from).collect();
    let serving = models
        .iter()
        .filter(|model| model.status == LlamaModelStatus::Loaded)
        .find_map(|model| model.instance.as_ref());
    let slots = match serving {
        Some(instance) => {
            http::get_json::<Vec<ListedSlot>>(&instance.url("slots"), &[], FETCH_TIMEOUT_SECS)
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
        scope: PhantomData,
    })
}

impl<S: UnitScope> Source for LlamaServer<S> {
    type Error = http::Error;
    type Key = LlamaServerUnit<S>;

    const NAME: &'static str = PROGRAM_NAME;
}

impl<S: UnitScope> LiveSource for LlamaServer<S> {
    fn events(server: LlamaServerUnit<S>) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(unit_running::<S>(server.unit.clone()), move || {
            let endpoint = server.endpoint.clone();
            unit_log_appends::<S>(server.unit.clone())
                .then(move |()| {
                    let endpoint = endpoint.clone();
                    async move { fetch_server::<S>(&endpoint).await }
                })
                .scan(None, |previous: &mut Option<Self>, fetched| {
                    let rated = fetched.map(|server| server.rated_since(previous.as_ref()));
                    if let Ok(server) = &rated {
                        *previous = Some(server.clone());
                    }
                    ready(Some(Delivery::Reading(rated)))
                })
                .boxed()
        })
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

impl Source for LlamaModelProps {
    type Error = http::Error;
    type Key = ModelInstance;

    const NAME: &'static str = "llama-server model";
}

impl LiveSource for LlamaModelProps {
    fn events(instance: ModelInstance) -> impl Stream<Item = Delivery<Self>> + Send {
        stream::once(async move {
            Delivery::Reading(http::get_json(&instance.url("props"), &[], FETCH_TIMEOUT_SECS).await)
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
                {"id":"Qwen3.8","status":{"value":"loaded","args":["llama-server","--host","127.0.0.1","--port","57883","--ctx-size","32768"]}}
            ]}"#,
        )
        .unwrap();
        let models: Vec<LlamaModel> = listed.data.into_iter().map(LlamaModel::from).collect();
        assert_eq!(models[0].status, LlamaModelStatus::Unloaded);
        assert_eq!(models[0].instance, None);
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
