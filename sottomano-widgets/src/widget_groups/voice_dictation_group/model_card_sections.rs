use sottomano_sources::voice_dictation_input::huggingface_model_card::HuggingFaceModelCard;

use super::speech_model_item::non_empty;
use crate::{
    clock::Clock,
    widgets::{
        localized_quantities::Quantity,
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, entry},
    },
};

impl ReadingSections for HuggingFaceModelCard {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("Likes", Quantity::count(self.likes))),
                    entry("Task", non_empty(&self.pipeline_tag)),
                    entry("Library", non_empty(&self.library_name)),
                    Some(KeyValue::new(
                        "Updated",
                        Cell::RelativeMoment(self.last_modified),
                    )),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Model card"),
        ]
    }
}
