use iced::{
    Color, Font,
    font::{Style as FontStyle, Weight},
    widget::{span, text::Span},
};
use quick_xml::{Reader, escape::resolve_xml_entity, events::Event, name::LocalName};

use crate::config::theme::font::ResolvedFont;

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

pub struct BodyParagraph<'a> {
    pub spans: Vec<Span<'a>>,
    pub preformatted: bool,
}

pub fn body_paragraphs<'a>(
    body: &str,
    color: Color,
    size: f32,
    font: &ResolvedFont,
    mono: Font,
) -> Vec<BodyParagraph<'a>> {
    let mut reader = Reader::from_str(body);
    let config = reader.config_mut();
    config.allow_dangling_amp = true;
    config.allow_unmatched_ends = true;
    config.check_end_names = false;

    let mut stack: Vec<String> = Vec::new();
    let mut paragraphs: Vec<BodyParagraph> = Vec::new();
    let mut push_text = |text: String, stack: &[String]| {
        let emphasis = Emphasis::from_stack(stack, font.iced_font, mono);
        let styled = span(text)
            .font(emphasis.font)
            .size(size)
            .color(color)
            .underline(emphasis.underline);
        let preformatted = stack.iter().any(|tag| tag == "pre");
        match paragraphs.last_mut() {
            Some(last) if last.preformatted == preformatted => last.spans.push(styled),
            _ => paragraphs.push(BodyParagraph {
                spans: vec![styled],
                preformatted,
            }),
        }
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => stack.push(lowercase_name(&tag.local_name())),
            Ok(Event::End(tag)) => {
                let name = lowercase_name(&tag.local_name());
                if let Some(pos) = stack.iter().rposition(|open| *open == name) {
                    stack.remove(pos);
                }
            }
            Ok(Event::Text(text)) => push_text((*text).to_owned(), &stack),
            Ok(Event::CData(text)) => {
                push_text((*text).to_owned(), &stack);
            }
            Ok(Event::GeneralRef(reference)) => {
                let name: &str = &reference;
                let resolved = match reference.resolve_char_ref() {
                    Ok(Some(character)) => character.to_string(),
                    _ => {
                        resolve_xml_entity(&name).map_or_else(|| format!("&{name};"), str::to_owned)
                    }
                };
                push_text(resolved, &stack);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                let unparsed_from = usize::try_from(reader.error_position()).unwrap_or(body.len());
                push_text(body[unparsed_from..].to_owned(), &stack);
                break;
            }
        }
    }
    for paragraph in &mut paragraphs {
        if let Some(first) = paragraph.spans.first_mut()
            && let Some(rest) = first
                .text
                .strip_prefix("\r\n")
                .or(first.text.strip_prefix('\n'))
        {
            first.text = rest.to_owned().into();
        }
        if let Some(last) = paragraph.spans.last_mut()
            && let Some(rest) = last
                .text
                .strip_suffix("\r\n")
                .or(last.text.strip_suffix('\n'))
        {
            last.text = rest.to_owned().into();
        }
        paragraph.spans.retain(|styled| !styled.text.is_empty());
    }
    paragraphs.retain(|paragraph| !paragraph.spans.is_empty());
    paragraphs
}

fn lowercase_name(name: &LocalName) -> String {
    name.as_ref().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use super::*;

    fn render(body: &str) -> String {
        let font = ResolvedFont {
            iced_font: Font::DEFAULT,
            text_size_px: 14.0,
            small_text_size_px: 12.0,
            icon_size_px: 14.0,
            data: Arc::from([].as_slice()),
            covered: Arc::new(HashSet::new()),
        };
        body_paragraphs(
            body,
            Color::BLACK,
            font.text_size_px,
            &font,
            Font::MONOSPACE,
        )
        .iter()
        .flat_map(|paragraph| &paragraph.spans)
        .map(|s| s.text.to_string())
        .collect()
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
