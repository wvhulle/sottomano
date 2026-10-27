use sottomano_sources::ai_usage_spend::gguf_header_model_card::{
    BaseModel, GgufModelCard, hugging_face_repository,
};

use crate::{
    clock::Clock,
    widgets::{
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, ReadingSections, Section},
    },
};

fn filled_text(text: &str) -> Option<&str> {
    Some(text).filter(|text| !text.is_empty())
}

fn hugging_face_link(label: String, url: &str) -> Cell {
    Cell::Link {
        label,
        url: url.to_owned(),
        glyph: Some(SimpleIcon::HuggingFace.into()),
    }
}

fn repository_link(url: &str) -> Option<Cell> {
    hugging_face_repository(url).map(|repository| hugging_face_link(repository.to_owned(), url))
}

fn base_model_row(base_model: &BaseModel) -> KeyValue {
    let label = [base_model.organization.as_str(), base_model.name.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let value = match hugging_face_repository(&base_model.repository_url) {
        Some(_) => hugging_face_link(label, &base_model.repository_url),
        None => Cell::from(label),
    };
    KeyValue::new("Base model", value)
}

fn license_cell(card: &GgufModelCard) -> Option<Cell> {
    let license = filled_text(&card.license)?;
    Some(match filled_text(&card.license_link) {
        Some(license_link) => Cell::Link {
            label: license.to_owned(),
            url: license_link.to_owned(),
            glyph: None,
        },
        None => Cell::from(license),
    })
}

impl ReadingSections for GgufModelCard {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let identity_rows = [
            filled_text(&self.name).map(|model_name| KeyValue::new("Name", model_name)),
            filled_text(&self.size_label)
                .map(|parameter_count| KeyValue::new("Parameters", parameter_count)),
            license_cell(self).map(|license_cell| KeyValue::new("License", license_cell)),
            filled_text(&self.quantized_by)
                .map(|quantizer_name| KeyValue::new("Quantized by", quantizer_name)),
            repository_link(&self.repository_url)
                .map(|repository_link| KeyValue::new("GGUF repository", repository_link)),
            repository_link(&self.source_url)
                .map(|source_link| KeyValue::new("Source", source_link)),
        ]
        .into_iter()
        .flatten();
        let lineage_rows = self.base_models.iter().map(base_model_row);
        let tags = (!self.tags.is_empty()).then(|| KeyValue::new("Tags", self.tags.join(", ")));
        let description = filled_text(&self.description)
            .map(|description| KeyValue::new("Description", Cell::Hint(description.to_owned())));
        vec![Section::dictionary(
            identity_rows
                .chain(lineage_rows)
                .chain(tags)
                .chain(description),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tooltip_body::SectionBody;

    #[test]
    fn base_models_link_to_their_hugging_face_repository() {
        let card = GgufModelCard {
            name: "Qwen3.6-27B".to_owned(),
            quantized_by: "Unsloth".to_owned(),
            base_models: vec![BaseModel {
                name: "Qwen3.6 27B".to_owned(),
                organization: "Qwen".to_owned(),
                repository_url: "https://huggingface.co/Qwen/Qwen3.6-27B".to_owned(),
            }],
            ..GgufModelCard::default()
        };
        let sections = card.sections(Clock::real());
        let SectionBody::Dictionary(rows) = sections[0].body() else {
            panic!("expected a dictionary");
        };
        let keys: Vec<&Cell> = rows.iter().map(KeyValue::key).collect();
        assert_eq!(
            keys,
            ["Name", "Quantized by", "Base model"]
                .map(Cell::from)
                .iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            rows[2].value(),
            &hugging_face_link(
                "Qwen Qwen3.6 27B".to_owned(),
                "https://huggingface.co/Qwen/Qwen3.6-27B"
            )
        );
    }
}
