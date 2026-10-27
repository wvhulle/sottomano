use sottomano_sources::voice_dictation_input::huggingface_model_card::HuggingFaceModelCard;

use crate::{
    clock::Clock,
    widgets::{
        localized_quantities::Quantity,
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, entry},
    },
};

fn filled_text(text: &str) -> Option<&str> {
    Some(text).filter(|text| !text.is_empty())
}

impl ReadingSections for HuggingFaceModelCard {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("Likes", Quantity::count(self.likes))),
                    entry("Task", filled_text(&self.pipeline_tag)),
                    entry("Library", filled_text(&self.library_name)),
                    Some(KeyValue::new(
                        "Updated",
                        Cell::RelativeMoment(self.last_modified),
                    )),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("On Hugging Face"),
        ]
    }
}
