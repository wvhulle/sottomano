use std::{fmt::Display, str::FromStr};

use winnow::{
    ModalResult, Parser,
    ascii::{multispace1, till_line_ending},
    combinator::{alt, cut_err, delimited, eof, preceded, repeat, terminated},
    error::{ContextError, ParseError, StrContext},
    token::{any, none_of, take_while},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyValuesValue {
    Text(String),
    Block(KeyValuesBlock),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyValuesBlock {
    pub entries: Vec<(String, KeyValuesValue)>,
}

#[derive(Debug, thiserror::Error)]
#[error("Valve KeyValues text malformed at byte {offset}: {reason}")]
pub struct KeyValuesSyntaxError {
    pub offset: usize,
    pub reason: String,
}

impl From<ParseError<&str, ContextError>> for KeyValuesSyntaxError {
    fn from(parse_failure: ParseError<&str, ContextError>) -> Self {
        KeyValuesSyntaxError {
            offset: parse_failure.offset(),
            reason: parse_failure.inner().to_string(),
        }
    }
}

impl KeyValuesBlock {
    fn values_named<'block, 'key>(
        &'block self,
        key: &'key str,
    ) -> impl Iterator<Item = &'block KeyValuesValue> + use<'block, 'key> {
        self.entries
            .iter()
            .filter(move |(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
            .map(|(_, value)| value)
    }

    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        self.values_named(key).find_map(|value| match value {
            KeyValuesValue::Text(text) => Some(text.as_str()),
            KeyValuesValue::Block(_) => None,
        })
    }

    #[must_use]
    pub fn block(&self, key: &str) -> Option<&KeyValuesBlock> {
        self.values_named(key).find_map(|value| match value {
            KeyValuesValue::Block(block) => Some(block),
            KeyValuesValue::Text(_) => None,
        })
    }

    #[must_use]
    pub fn parsed<T>(&self, key: &str) -> Option<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.text(key)?
            .parse()
            .inspect_err(|parse_failure| {
                tracing::debug!("Valve KeyValues entry {key} unreadable: {parse_failure}");
            })
            .ok()
    }

    pub fn blocks(&self) -> impl Iterator<Item = (&str, &KeyValuesBlock)> {
        self.entries.iter().filter_map(|(key, value)| match value {
            KeyValuesValue::Block(block) => Some((key.as_str(), block)),
            KeyValuesValue::Text(_) => None,
        })
    }

    pub fn texts(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().filter_map(|(key, value)| match value {
            KeyValuesValue::Text(text) => Some((key.as_str(), text.as_str())),
            KeyValuesValue::Block(_) => None,
        })
    }
}

fn line_comment(input: &mut &str) -> ModalResult<()> {
    preceded(('/', '/'), till_line_ending)
        .void()
        .parse_next(input)
}

fn insignificant(input: &mut &str) -> ModalResult<()> {
    repeat(0.., alt((multispace1.void(), line_comment))).parse_next(input)
}

fn escaped_character(input: &mut &str) -> ModalResult<char> {
    preceded(
        '\\',
        any.map(|escaped| match escaped {
            'n' => '\n',
            't' => '\t',
            other => other,
        }),
    )
    .parse_next(input)
}

fn quoted_text(input: &mut &str) -> ModalResult<String> {
    delimited(
        '"',
        repeat(0.., alt((escaped_character, none_of(['"', '\\'])))),
        cut_err('"').context(StrContext::Label("closing quote")),
    )
    .parse_next(input)
}

fn bare_text(input: &mut &str) -> ModalResult<String> {
    take_while(1.., |character: char| {
        !character.is_whitespace() && !matches!(character, '"' | '{' | '}')
    })
    .map(str::to_owned)
    .parse_next(input)
}

fn token_text(input: &mut &str) -> ModalResult<String> {
    alt((quoted_text, bare_text)).parse_next(input)
}

fn nested_block(input: &mut &str) -> ModalResult<KeyValuesBlock> {
    delimited(
        '{',
        block_entries,
        cut_err(preceded(insignificant, '}')).context(StrContext::Label("closing brace")),
    )
    .parse_next(input)
}

fn entry_value(input: &mut &str) -> ModalResult<KeyValuesValue> {
    alt((
        nested_block.map(KeyValuesValue::Block),
        token_text.map(KeyValuesValue::Text),
    ))
    .parse_next(input)
}

fn block_entry(input: &mut &str) -> ModalResult<(String, KeyValuesValue)> {
    (
        preceded(insignificant, token_text),
        preceded(insignificant, entry_value),
    )
        .parse_next(input)
}

fn block_entries(input: &mut &str) -> ModalResult<KeyValuesBlock> {
    repeat(0.., block_entry)
        .map(|entries| KeyValuesBlock { entries })
        .parse_next(input)
}

pub fn parse_keyvalues_document(text: &str) -> Result<KeyValuesBlock, KeyValuesSyntaxError> {
    terminated(block_entries, (insignificant, eof))
        .parse(text)
        .map_err(KeyValuesSyntaxError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY_FOLDERS: &str = r#""libraryfolders"
{
	"0"
	{
		"path"		"/home/player/.local/share/Steam"
		"label"		""
		"apps"
		{
			"620"		"12792163663"
			"1091500"		"66818683782"
		}
	}
}
"#;

    #[test]
    fn library_folders_nest_apps_under_each_folder() {
        let document = parse_keyvalues_document(LIBRARY_FOLDERS).expect("valid document");
        let folder = document
            .block("libraryfolders")
            .and_then(|folders| folders.block("0"))
            .expect("first folder");
        assert_eq!(folder.text("path"), Some("/home/player/.local/share/Steam"));
        assert_eq!(folder.text("label"), Some(""));
        let sizes: Vec<(&str, u64)> = folder
            .block("apps")
            .expect("apps block")
            .texts()
            .map(|(app_id, size)| (app_id, size.parse().expect("numeric size")))
            .collect();
        assert_eq!(
            sizes,
            [("620", 12_792_163_663), ("1091500", 66_818_683_782)]
        );
    }

    #[test]
    fn app_manifest_fields_parse_into_numbers() {
        let manifest = parse_keyvalues_document(
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1091500\"\n\t\"name\"\t\t\"Cyberpunk \
             2077\"\n\t\"StateFlags\"\t\t\"4\"\n}\n",
        )
        .expect("valid manifest");
        let app_state = manifest.block("AppState").expect("app state");
        assert_eq!(app_state.parsed::<u32>("appid"), Some(1_091_500));
        assert_eq!(app_state.text("name"), Some("Cyberpunk 2077"));
        assert_eq!(app_state.parsed::<u32>("stateflags"), Some(4));
    }

    #[test]
    fn escapes_and_comments_are_understood() {
        let document = parse_keyvalues_document(
            "\u{2f}\u{2f} header comment\n\"root\" { \"quote\" \"say \\\"hi\\\"\" \u{2f}\u{2f} \
             trailing\n bare value }",
        )
        .expect("valid document");
        let root = document.block("root").expect("root block");
        assert_eq!(root.text("quote"), Some("say \"hi\""));
        assert_eq!(root.text("bare"), Some("value"));
    }

    #[test]
    fn an_unclosed_block_is_reported_with_its_offset() {
        let failure =
            parse_keyvalues_document("\"root\" { \"key\" \"value\"").expect_err("unclosed");
        assert!(failure.reason.contains("closing brace"), "{failure}");
    }
}
