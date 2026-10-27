use chrono::{DateTime, Utc};
use futures::{Stream, stream};
use serde::Deserialize;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, system_protocol_clients::http_json_client,
};

const FETCH_TIMEOUT_SECS: u64 = 5;
const HUGGING_FACE_MODELS_API: &str = "https://huggingface.co/api/models";

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct ModelCardData {
    #[serde(default)]
    pub license: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HuggingFaceModelCard {
    #[serde(default)]
    pub likes: u64,
    #[serde(default, rename = "pipeline_tag")]
    pub pipeline_tag: String,
    #[serde(default, rename = "library_name")]
    pub library_name: String,
    pub last_modified: DateTime<Utc>,
    #[serde(default)]
    pub card_data: ModelCardData,
}

#[must_use]
pub fn model_card_url(repository: &str) -> String {
    format!("{HUGGING_FACE_MODELS_API}/{repository}")
}

impl ReadingSource for HuggingFaceModelCard {
    type Error = http_json_client::Error;
    type StreamInputs = String;

    const NAME: &'static str = "Hugging Face model card";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::ReadOnce;

    fn delivery_stream(repository: String) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(async move {
            SourceDelivery::Reading(
                http_json_client::get_json(&model_card_url(&repository), &[], FETCH_TIMEOUT_SECS)
                    .await,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_card_reads_the_fields_the_api_returns_and_defaults_the_rest() {
        let card: HuggingFaceModelCard = serde_json::from_str(
            r#"{
                "id": "ggerganov/whisper.cpp",
                "downloads": 125000,
                "likes": 1100,
                "pipeline_tag": "automatic-speech-recognition",
                "lastModified": "2024-10-02T09:38:51.000Z",
                "cardData": {"license": "mit"},
                "tags": ["whisper"]
            }"#,
        )
        .expect("the model card parses");
        assert_eq!(card.likes, 1100);
        assert_eq!(card.pipeline_tag, "automatic-speech-recognition");
        assert_eq!(card.library_name, "");
        assert_eq!(card.card_data.license, "mit");
    }

    #[test]
    fn model_card_url_points_at_the_models_api() {
        assert_eq!(
            model_card_url("ggerganov/whisper.cpp"),
            "https://huggingface.co/api/models/ggerganov/whisper.cpp"
        );
    }
}
