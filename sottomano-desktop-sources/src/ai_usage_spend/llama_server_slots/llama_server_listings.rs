use std::{path::PathBuf, time::Instant};

use serde::Deserialize;

use super::{
    ChildInstance, DecodeRate, LlamaModel, LlamaModelStatus, LlamaServer, LlamaSlot,
    ModelFileArgument, ModelInstance, SlotAssignment, SlotTask, SlotWork,
};
use crate::system_protocol_clients::http_json_client;

#[derive(Deserialize)]
pub(super) struct ModelList {
    pub(super) data: Vec<ListedModel>,
}

#[derive(Deserialize)]
pub(super) struct ListedModel {
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

    fn instance(&self) -> ChildInstance {
        self.listening_instance()
            .map_or(ChildInstance::NotListening, ChildInstance::Listening)
    }

    fn listening_instance(&self) -> Option<ModelInstance> {
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
        let instance = match listed.status.value {
            LlamaModelStatus::Loaded | LlamaModelStatus::Sleeping => listed.status.instance(),
            LlamaModelStatus::Downloading
            | LlamaModelStatus::Downloaded
            | LlamaModelStatus::Unloaded
            | LlamaModelStatus::Loading => ChildInstance::NotListening,
        };
        let model_file = listed
            .status
            .argument("--model")
            .map_or(ModelFileArgument::NotGiven, |model_path| {
                ModelFileArgument::Given(PathBuf::from(model_path))
            });
        Self {
            id: listed.id,
            status: listed.status.value,
            instance,
            model_file,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct ListedSlot {
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
            })
            .map_or(SlotAssignment::Unassigned, SlotAssignment::Assigned);
        let work = if listed.is_processing {
            SlotWork::Processing
        } else {
            SlotWork::Idle
        };
        Self {
            id: listed.id,
            context: listed.n_ctx,
            work,
            task,
        }
    }
}

pub(super) async fn fetch_server(endpoint: &str) -> Result<LlamaServer, http_json_client::Error> {
    let listed: ModelList =
        http_json_client::get_json(&format!("{endpoint}/v1/models"), &[]).await?;
    let models: Vec<LlamaModel> = listed.data.into_iter().map(LlamaModel::from).collect();
    let serving = models
        .iter()
        .filter(|model| model.status == LlamaModelStatus::Loaded)
        .find_map(|model| match &model.instance {
            ChildInstance::Listening(instance) => Some(instance),
            ChildInstance::NotListening => None,
        });
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
        generation_rate: DecodeRate::Unmeasured,
        sampled_at: Instant::now(),
    })
}
