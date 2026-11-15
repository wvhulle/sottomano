use sottomano_desktop_sources::voice_dictation_input::huggingface_model_card::HuggingFaceModelCard;

use crate::{
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

fn filled_text(text: &str) -> Option<&str> {
    Some(text).filter(|text| !text.is_empty())
}

#[must_use]
pub fn hugging_face_model_card_sections(
    hugging_face_model_card: &HuggingFaceModelCard,
) -> Vec<Section> {
    vec![Section::new(
        "Model",
        dictionary_table(
            [
                Some(LabeledValue::new(
                    "Likes",
                    Quantity::count(hugging_face_model_card.likes),
                )),
                filled_text(&hugging_face_model_card.pipeline_tag)
                    .map(|pipeline_task| LabeledValue::new("Task", pipeline_task)),
                filled_text(&hugging_face_model_card.library_name)
                    .map(|library_name| LabeledValue::new("Library", library_name)),
                Some(LabeledValue::new(
                    "Updated",
                    ValueCell::RelativeMoment(hugging_face_model_card.last_modified),
                )),
            ]
            .into_iter()
            .flatten(),
        ),
    )]
}
