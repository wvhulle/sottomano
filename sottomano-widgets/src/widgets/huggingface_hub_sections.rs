use sottomano_sources::voice_dictation_input::huggingface_model_card::HuggingFaceModelCard;

use crate::{
    clock::Clock,
    hover_cards::hover_card_body::{ReadingSections, Section},
    widgets::{
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

fn filled_text(text: &str) -> Option<&str> {
    Some(text).filter(|text| !text.is_empty())
}

impl ReadingSections for HuggingFaceModelCard {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(
            [
                Some(LabeledValue::new("Likes", Quantity::count(self.likes))),
                filled_text(&self.pipeline_tag)
                    .map(|pipeline_task| LabeledValue::new("Task", pipeline_task)),
                filled_text(&self.library_name)
                    .map(|library_name| LabeledValue::new("Library", library_name)),
                Some(LabeledValue::new(
                    "Updated",
                    ValueCell::RelativeMoment(self.last_modified),
                )),
            ]
            .into_iter()
            .flatten(),
        )]
    }
}
