use iced::{
    Color, Font,
    font::{Style as FontStyle, Weight},
    widget::{span, text::Span},
};
use quick_xml::{Reader, escape::resolve_xml_entity, events::Event, name::LocalName};
use sottomano_config::theme::font::ResolvedFont;

use crate::widgets::text::FontFaces;

#[derive(Clone, Copy)]
struct Emphasis {
    font: Font,
    underline: bool,
}

impl Emphasis {
    fn from_stack(stack: &[String], base: Font, mono: Font) -> Self {
        let mut emphasis = Self {
            font: base,
            underline: false,
        };
        for tag in stack {
            match tag.as_str() {
                "b" => emphasis.font.weight = Weight::Bold,
                "i" => emphasis.font.style = FontStyle::Italic,
                "u" => emphasis.underline = true,
                "pre" | "tt" => emphasis.font.family = mono.family,
                _ => {}
            }
        }
        emphasis
    }
}

#[derive(Clone)]
pub struct BodyRow<'a> {
    pub key: Vec<Span<'a>>,
    pub value: Vec<Span<'a>>,
}

pub enum BodyBlock<'a> {
    Paragraph {
        spans: Vec<Span<'a>>,
        preformatted: bool,
    },
    Rows(Vec<BodyRow<'a>>),
}

struct BlockBuilder<'a> {
    blocks: Vec<BodyBlock<'a>>,
}

impl<'a> BlockBuilder<'a> {
    fn open_rows(&mut self) {
        self.blocks.push(BodyBlock::Rows(Vec::new()));
    }

    fn open_row(&mut self) {
        if let Some(BodyBlock::Rows(rows)) = self.blocks.last_mut() {
            rows.push(BodyRow {
                key: Vec::new(),
                value: Vec::new(),
            });
        }
    }

    fn push_span(&mut self, styled: Span<'a>, stack: &[String]) {
        let inside = |tag: &str| stack.iter().any(|open| open == tag);
        if inside("dl") {
            if let Some(BodyBlock::Rows(rows)) = self.blocks.last_mut()
                && let Some(row) = rows.last_mut()
            {
                if inside("dt") {
                    row.key.push(styled);
                } else if inside("dd") {
                    row.value.push(styled);
                }
            }
            return;
        }
        let preformatted = inside("pre");
        match self.blocks.last_mut() {
            Some(BodyBlock::Paragraph {
                spans,
                preformatted: open,
            }) if *open == preformatted => spans.push(styled),
            _ => self.blocks.push(BodyBlock::Paragraph {
                spans: vec![styled],
                preformatted,
            }),
        }
    }

}

pub fn body_blocks<'a>(
    body: &str,
    color: Color,
    size: f32,
    font: &ResolvedFont,
    mono: Font,
) -> Vec<BodyBlock<'a>> {
    let mut reader = Reader::from_str(body);
    let config = reader.config_mut();
    config.allow_dangling_amp = true;
    config.allow_unmatched_ends = true;
    config.check_end_names = false;

    let mut stack: Vec<String> = Vec::new();
    let mut builder = BlockBuilder { blocks: Vec::new() };
    let styled = |text: String, stack: &[String]| {
        let emphasis = Emphasis::from_stack(stack, FontFaces::from(font).regular, mono);
        span(text)
            .font(emphasis.font)
            .size(size)
            .color(color)
            .underline(emphasis.underline)
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                let name = lowercase_name(&tag.local_name());
                match name.as_str() {
                    "dl" => builder.open_rows(),
                    "dt" => builder.open_row(),
                    _ => {}
                }
                stack.push(name);
            }
            Ok(Event::End(tag)) => {
                let name = lowercase_name(&tag.local_name());
                if let Some(pos) = stack.iter().rposition(|open| *open == name) {
                    stack.remove(pos);
                }
            }
            Ok(Event::Text(text)) => builder.push_span(styled((*text).to_owned(), &stack), &stack),
            Ok(Event::CData(text)) => {
                builder.push_span(styled((*text).to_owned(), &stack), &stack);
            }
            Ok(Event::GeneralRef(reference)) => {
                let name: &str = &reference;
                let resolved = match reference.resolve_char_ref() {
                    Ok(Some(character)) => character.to_string(),
                    _ => {
                        resolve_xml_entity(name).map_or_else(|| format!("&{name};"), str::to_owned)
                    }
                };
                builder.push_span(styled(resolved, &stack), &stack);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                let unparsed_from = usize::try_from(reader.error_position()).unwrap_or(body.len());
                builder.push_span(styled(body[unparsed_from..].to_owned(), &stack), &stack);
                break;
            }
        }
    }
    builder.blocks
}

fn lowercase_name(name: &LocalName) -> String {
    name.as_ref().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sottomano_config::theme::font::FontRole;

    use super::*;

    fn blocks(body: &str) -> Vec<BodyBlock<'static>> {
        let font = ResolvedFont {
            role: FontRole::Text,
            family: String::from("sans-serif"),
            text_size_px: 14.0,
            small_text_size_px: 12.0,
            icon_size_px: 14.0,
            data: Arc::from([].as_slice()),
        };
        body_blocks(
            body,
            Color::BLACK,
            font.text_size_px,
            &font,
            Font::MONOSPACE,
        )
    }

    fn joined(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.text.to_string()).collect()
    }

    fn render(body: &str) -> String {
        blocks(body)
            .iter()
            .map(|block| match block {
                BodyBlock::Paragraph { spans, .. } => joined(spans),
                BodyBlock::Rows(rows) => rows
                    .iter()
                    .map(|row| format!("{}={}", joined(&row.key), joined(&row.value)))
                    .collect::<Vec<_>>()
                    .join(";"),
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn definition_list_becomes_rows() {
        assert_eq!(
            render("<dl>\n<dt>Used</dt><dd>76%</dd>\n<dt>Resets</dt><dd>Friday 04:00</dd>\n</dl>"),
            "Used=76%;Resets=Friday 04:00"
        );
    }

    #[test]
    fn paragraphs_around_a_definition_list_stay_separate() {
        assert_eq!(
            render("<b>Weekly</b><dl><dt>Used</dt><dd>76%</dd></dl>after"),
            "Weekly|Used=76%|after"
        );
    }

    #[test]
    fn monospace_inside_a_row_value_keeps_its_family() {
        let parsed = blocks("<dl><dt>Core</dt><dd><tt>/var/lib/core</tt></dd></dl>");
        let Some(BodyBlock::Rows(rows)) = parsed.first() else {
            panic!("expected rows");
        };
        assert_eq!(
            rows[0].value[0].font.unwrap().family,
            Font::MONOSPACE.family
        );
    }

    #[test]
    fn strips_tags_keeps_text() {
        assert_eq!(render("<b>Build failed</b>"), "Build failed");
        assert_eq!(render("plain"), "plain");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(render("a &amp; b &lt;c&gt;"), "a & b <c>");
        assert_eq!(render("&#65;&#x42;"), "AB");
    }

    #[test]
    fn unknown_tag_stripped_text_kept() {
        assert_eq!(render("<span foreground='red'>hi</span>"), "hi");
    }
}
