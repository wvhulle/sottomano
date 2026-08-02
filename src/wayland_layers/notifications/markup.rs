use iced::{
    Color, Font,
    font::{Style as FontStyle, Weight},
    widget::{span, text::Span},
};

use crate::config::resolved::ResolvedFontConfig;

#[derive(Default, Clone, Copy)]
struct Emphasis {
    bold: bool,
    italic: bool,
    underline: bool,
    mono: bool,
}

impl Emphasis {
    fn from_stack(stack: &[String]) -> Self {
        let mut e = Emphasis::default();
        for tag in stack {
            match tag.as_str() {
                "b" => e.bold = true,
                "i" => e.italic = true,
                "u" => e.underline = true,
                "pre" | "tt" => e.mono = true,
                _ => {}
            }
        }
        e
    }

    fn font(self, base: Font, mono: Font) -> Font {
        let mut font = if self.mono { mono } else { base };
        if self.bold {
            font.weight = Weight::Bold;
        }
        if self.italic {
            font.style = FontStyle::Italic;
        }
        font
    }
}

enum Token {
    Text(String),
    Open(String),
    Close(String),
}

pub fn body_spans<'a>(
    body: &str,
    color: Color,
    size: f32,
    font: &ResolvedFontConfig,
    mono: Font,
) -> Vec<Span<'a>> {
    let mut stack: Vec<String> = Vec::new();
    let mut spans = Vec::new();
    for token in tokenize(body) {
        match token {
            Token::Text(text) => {
                let emphasis = Emphasis::from_stack(&stack);
                spans.push(
                    span(text)
                        .font(emphasis.font(font.iced_font, mono))
                        .size(size)
                        .color(color)
                        .underline(emphasis.underline),
                );
            }
            Token::Open(name) => stack.push(name),
            Token::Close(name) => {
                if let Some(pos) = stack.iter().rposition(|t| *t == name) {
                    stack.remove(pos);
                }
            }
        }
    }
    spans
}

fn tokenize(body: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, tokens: &mut Vec<Token>| {
        if !text.is_empty() {
            tokens.push(Token::Text(std::mem::take(text)));
        }
    };

    let mut rest = body;
    while let Some(at) = rest.find(['<', '&']) {
        let (before, marker) = rest.split_at(at);
        text.push_str(before);
        match marker.as_bytes()[0] {
            b'<' => {
                if let Some(end) = marker.find('>') {
                    flush(&mut text, &mut tokens);
                    if let Some(token) = tag_token(marker[1..end].trim()) {
                        tokens.push(token);
                    }
                    rest = &marker[end + 1..];
                } else {
                    text.push('<');
                    rest = &marker[1..];
                }
            }
            _ => {
                if let Some((semi, decoded)) = marker
                    .find(';')
                    .and_then(|semi| decode_entity(&marker[1..semi]).map(|decoded| (semi, decoded)))
                {
                    text.push(decoded);
                    rest = &marker[semi + 1..];
                } else {
                    text.push('&');
                    rest = &marker[1..];
                }
            }
        }
    }
    text.push_str(rest);
    flush(&mut text, &mut tokens);
    tokens
}

fn tag_token(tag: &str) -> Option<Token> {
    if let Some(name) = tag.strip_prefix('/') {
        return Some(Token::Close(name.trim().to_ascii_lowercase()));
    }
    if tag.ends_with('/') {
        return None;
    }
    let name = tag.split_whitespace().next()?.to_ascii_lowercase();
    (!name.is_empty()).then_some(Token::Open(name))
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = entity.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(body: &str) -> String {
        let font = ResolvedFontConfig {
            iced_font: Font::DEFAULT,
            text_size_px: 14.0,
            icon_size_px: 14.0,
            data: std::sync::Arc::from([].as_slice()),
            covered: std::sync::Arc::new(std::collections::HashSet::new()),
        };
        body_spans(
            body,
            Color::BLACK,
            font.text_size_px,
            &font,
            Font::MONOSPACE,
        )
        .iter()
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
