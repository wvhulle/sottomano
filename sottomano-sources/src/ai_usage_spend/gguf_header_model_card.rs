use std::{
    collections::BTreeMap,
    fmt,
    fs::File,
    io::{self, BufReader, Read, Seek},
    path::{Path, PathBuf},
};

use futures::{Stream, stream};
use tokio::task::spawn_blocking;

use crate::{SnapshotDelivery, SnapshotSource};

const GGUF_MAGIC: &[u8; 4] = b"GGUF";
const OLDEST_READABLE_VERSION: u32 = 2;
const GENERAL_KEY_PREFIX: &str = "general.";
const HUGGING_FACE_SITE: &str = "https://huggingface.co/";
const STRING_VALUE_TYPE: u32 = 8;
const ARRAY_VALUE_TYPE: u32 = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseModel {
    pub name: String,
    pub organization: String,
    pub repository_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GgufModelCard {
    pub name: String,
    pub size_label: String,
    pub license: String,
    pub license_link: String,
    pub quantized_by: String,
    pub description: String,
    pub repository_url: String,
    pub source_url: String,
    pub base_models: Vec<BaseModel>,
    pub tags: Vec<String>,
}

impl GgufModelCard {
    #[must_use]
    pub fn base_model_repository(&self) -> Option<&str> {
        self.base_models
            .iter()
            .find_map(|base_model| hugging_face_repository(&base_model.repository_url))
    }
}

#[must_use]
pub fn hugging_face_repository(url: &str) -> Option<&str> {
    url.strip_prefix(HUGGING_FACE_SITE)
        .map(|repository| repository.trim_end_matches('/'))
        .filter(|repository| repository.contains('/'))
}

#[derive(Debug, Clone)]
pub enum GgufHeaderError {
    Unreadable(String),
    NotGguf,
    UnsupportedVersion(u32),
    UnknownValueType(u32),
}

impl fmt::Display for GgufHeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GgufHeaderError::Unreadable(reason) => {
                write!(formatter, "model file unreadable: {reason}")
            }
            GgufHeaderError::NotGguf => formatter.write_str("model file is not a GGUF file"),
            GgufHeaderError::UnsupportedVersion(version) => {
                write!(formatter, "GGUF version {version} is not supported")
            }
            GgufHeaderError::UnknownValueType(value_type) => {
                write!(
                    formatter,
                    "GGUF metadata uses unknown value type {value_type}"
                )
            }
        }
    }
}

impl From<io::Error> for GgufHeaderError {
    fn from(error: io::Error) -> Self {
        GgufHeaderError::Unreadable(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum GeneralValue {
    Text(String),
    TextList(Vec<String>),
    Other,
}

struct HeaderReader<Source> {
    source: Source,
}

impl<Source: Read + Seek> HeaderReader<Source> {
    fn bytes<const LENGTH: usize>(&mut self) -> Result<[u8; LENGTH], GgufHeaderError> {
        let mut buffer = [0; LENGTH];
        self.source.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn unsigned_32(&mut self) -> Result<u32, GgufHeaderError> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }

    fn unsigned_64(&mut self) -> Result<u64, GgufHeaderError> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }

    fn skip(&mut self, byte_count: u64) -> Result<(), GgufHeaderError> {
        let offset = i64::try_from(byte_count).map_err(|_| GgufHeaderError::NotGguf)?;
        self.source.seek_relative(offset)?;
        Ok(())
    }

    fn text(&mut self) -> Result<String, GgufHeaderError> {
        let length = usize::try_from(self.unsigned_64()?).map_err(|_| GgufHeaderError::NotGguf)?;
        let mut buffer = vec![0; length];
        self.source.read_exact(&mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }

    fn skip_text(&mut self) -> Result<(), GgufHeaderError> {
        let length = self.unsigned_64()?;
        self.skip(length)
    }

    fn skip_value(&mut self, value_type: u32) -> Result<(), GgufHeaderError> {
        match value_type {
            STRING_VALUE_TYPE => self.skip_text(),
            ARRAY_VALUE_TYPE => {
                let element_type = self.unsigned_32()?;
                let element_count = self.unsigned_64()?;
                match fixed_value_width(element_type) {
                    Some(element_width) => self.skip(element_width * element_count),
                    None => (0..element_count).try_for_each(|_| self.skip_value(element_type)),
                }
            }
            scalar_type => {
                let width = fixed_value_width(scalar_type)
                    .ok_or(GgufHeaderError::UnknownValueType(scalar_type))?;
                self.skip(width)
            }
        }
    }

    fn general_value(&mut self, value_type: u32) -> Result<GeneralValue, GgufHeaderError> {
        match value_type {
            STRING_VALUE_TYPE => Ok(GeneralValue::Text(self.text()?)),
            ARRAY_VALUE_TYPE => {
                let element_type = self.unsigned_32()?;
                let element_count = self.unsigned_64()?;
                if element_type == STRING_VALUE_TYPE {
                    Ok(GeneralValue::TextList(
                        (0..element_count)
                            .map(|_| self.text())
                            .collect::<Result<_, _>>()?,
                    ))
                } else {
                    (0..element_count).try_for_each(|_| self.skip_value(element_type))?;
                    Ok(GeneralValue::Other)
                }
            }
            scalar_type => {
                self.skip_value(scalar_type)?;
                Ok(GeneralValue::Other)
            }
        }
    }

    fn general_metadata(&mut self) -> Result<BTreeMap<String, GeneralValue>, GgufHeaderError> {
        if &self.bytes::<4>()? != GGUF_MAGIC {
            return Err(GgufHeaderError::NotGguf);
        }
        let version = self.unsigned_32()?;
        if version < OLDEST_READABLE_VERSION {
            return Err(GgufHeaderError::UnsupportedVersion(version));
        }
        let _tensor_count = self.unsigned_64()?;
        let metadata_count = self.unsigned_64()?;
        let mut general = BTreeMap::new();
        for _ in 0..metadata_count {
            let key = self.text()?;
            let value_type = self.unsigned_32()?;
            if key.starts_with(GENERAL_KEY_PREFIX) {
                general.insert(key, self.general_value(value_type)?);
            } else {
                self.skip_value(value_type)?;
            }
        }
        Ok(general)
    }
}

fn fixed_value_width(value_type: u32) -> Option<u64> {
    match value_type {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

fn model_card_from(general: &BTreeMap<String, GeneralValue>) -> GgufModelCard {
    let text = |key: &str| match general.get(&format!("{GENERAL_KEY_PREFIX}{key}")) {
        Some(GeneralValue::Text(value)) => value.clone(),
        Some(GeneralValue::TextList(_) | GeneralValue::Other) | None => String::new(),
    };
    let base_model_count = general
        .keys()
        .filter(|key| {
            key.strip_prefix("general.base_model.")
                .and_then(|indexed_field| indexed_field.strip_suffix(".name"))
                .is_some()
        })
        .count();
    let base_models = (0..base_model_count)
        .map(|index| BaseModel {
            name: text(&format!("base_model.{index}.name")),
            organization: text(&format!("base_model.{index}.organization")),
            repository_url: text(&format!("base_model.{index}.repo_url")),
        })
        .collect();
    let repository_url = [text("url"), text("repo_url")]
        .into_iter()
        .find(|url| hugging_face_repository(url).is_some())
        .unwrap_or_default();
    GgufModelCard {
        name: text("name"),
        size_label: text("size_label"),
        license: text("license"),
        license_link: text("license.link"),
        quantized_by: text("quantized_by"),
        description: text("description"),
        repository_url,
        source_url: text("source.url"),
        base_models,
        tags: match general.get("general.tags") {
            Some(GeneralValue::TextList(tags)) => tags.clone(),
            Some(GeneralValue::Text(_) | GeneralValue::Other) | None => Vec::new(),
        },
    }
}

pub fn read_model_card(model_file: &Path) -> Result<GgufModelCard, GgufHeaderError> {
    let mut reader = HeaderReader {
        source: BufReader::new(File::open(model_file)?),
    };
    Ok(model_card_from(&reader.general_metadata()?))
}

impl SnapshotSource for GgufModelCard {
    type Error = GgufHeaderError;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "GGUF model card";

    fn delivery_stream(model_file: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(async move {
            let read = spawn_blocking(move || read_model_card(&model_file))
                .await
                .unwrap_or_else(|join_error| {
                    Err(GgufHeaderError::Unreadable(join_error.to_string()))
                });
            SnapshotDelivery::single_read(read)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn encoded_text(text: &str) -> Vec<u8> {
        (text.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(text.bytes())
            .collect()
    }

    fn text_entry(key: &str, value: &str) -> Vec<u8> {
        [
            encoded_text(key),
            STRING_VALUE_TYPE.to_le_bytes().to_vec(),
            encoded_text(value),
        ]
        .concat()
    }

    fn gguf_header(entries: &[Vec<u8>]) -> Vec<u8> {
        [
            GGUF_MAGIC.to_vec(),
            3_u32.to_le_bytes().to_vec(),
            0_u64.to_le_bytes().to_vec(),
            (entries.len() as u64).to_le_bytes().to_vec(),
            entries.concat(),
        ]
        .concat()
    }

    fn card_of(header: Vec<u8>) -> Result<GgufModelCard, GgufHeaderError> {
        let mut reader = HeaderReader {
            source: Cursor::new(header),
        };
        Ok(model_card_from(&reader.general_metadata()?))
    }

    #[test]
    fn general_keys_fill_the_card_and_other_keys_are_skipped() {
        let token_list = [
            encoded_text("tokenizer.ggml.tokens"),
            ARRAY_VALUE_TYPE.to_le_bytes().to_vec(),
            STRING_VALUE_TYPE.to_le_bytes().to_vec(),
            2_u64.to_le_bytes().to_vec(),
            encoded_text("<s>"),
            encoded_text("</s>"),
        ]
        .concat();
        let context_length = [
            encoded_text("qwen35.context_length"),
            4_u32.to_le_bytes().to_vec(),
            262_144_u32.to_le_bytes().to_vec(),
        ]
        .concat();
        let tags = [
            encoded_text("general.tags"),
            ARRAY_VALUE_TYPE.to_le_bytes().to_vec(),
            STRING_VALUE_TYPE.to_le_bytes().to_vec(),
            1_u64.to_le_bytes().to_vec(),
            encoded_text("unsloth"),
        ]
        .concat();
        let card = card_of(gguf_header(&[
            text_entry("general.name", "Qwen3.6-27B"),
            context_length,
            text_entry("general.size_label", "27B"),
            text_entry("general.quantized_by", "Unsloth"),
            text_entry("general.repo_url", "https://huggingface.co/unsloth"),
            text_entry("general.base_model.0.name", "Qwen3.6 27B"),
            text_entry("general.base_model.0.organization", "Qwen"),
            text_entry(
                "general.base_model.0.repo_url",
                "https://huggingface.co/Qwen/Qwen3.6-27B",
            ),
            token_list,
            tags,
        ]))
        .expect("the header parses");
        assert_eq!(card.name, "Qwen3.6-27B");
        assert_eq!(card.size_label, "27B");
        assert_eq!(card.quantized_by, "Unsloth");
        assert_eq!(card.repository_url, "");
        assert_eq!(card.tags, ["unsloth"]);
        assert_eq!(card.base_model_repository(), Some("Qwen/Qwen3.6-27B"));
    }

    #[test]
    fn files_without_the_gguf_magic_are_rejected() {
        assert!(matches!(
            card_of(b"GGML\x03\x00\x00\x00".to_vec()),
            Err(GgufHeaderError::NotGguf)
        ));
    }

    #[test]
    fn repository_urls_must_name_an_owner_and_a_model() {
        assert_eq!(
            hugging_face_repository("https://huggingface.co/mradermacher/gemma-4-GGUF"),
            Some("mradermacher/gemma-4-GGUF")
        );
        assert_eq!(
            hugging_face_repository("https://huggingface.co/unsloth"),
            None
        );
    }
}
