//! The Pango/HTML subset notification bodies use (`<b>`, `<i>`, `<u>`,
//! `<pre>`/`<tt>`), rendered as iced `rich_text` spans. Unknown tags are
//! stripped (their text kept); entities are decoded. Nothing is ever fetched —
//! this is the safe subset, never `RichText` that could load a remote `<img>`.

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

/// Render `body` into styled spans coloured `color` at `size`, using
/// `font.iced_font` for prose and `mono` for `<pre>`/`<tt>` runs.
pub fn body_spans<'a>(
    body: &str,
    color: Color,
    size: f32,
    font: &ResolvedFontConfig,
    mono: Font,
) -> Vec<Span<'a>> {
    let chars: Vec<char> = body.chars().collect();
    let mut spans = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '<' => {
                if let Some(close) = chars[i + 1..].iter().position(|&c| c == '>') {
                    flush(&mut text, &stack, color, size, font, mono, &mut spans);
                    let tag: String = chars[i + 1..i + 1 + close].iter().collect();
                    apply_tag(tag.trim(), &mut stack);
                    i += close + 2;
                } else {
                    text.push('<');
                    i += 1;
                }
            }
            '&' => {
                if let Some(semi) = chars[i + 1..].iter().position(|&c| c == ';') {
                    let entity: String = chars[i + 1..i + 1 + semi].iter().collect();
                    if let Some(decoded) = decode_entity(&entity) {
                        text.push(decoded);
                        i += semi + 2;
                        continue;
                    }
                }
                text.push('&');
                i += 1;
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    flush(&mut text, &stack, color, size, font, mono, &mut spans);
    spans
}

fn flush<'a>(
    text: &mut String,
    stack: &[String],
    color: Color,
    size: f32,
    font: &ResolvedFontConfig,
    mono: Font,
    spans: &mut Vec<Span<'a>>,
) {
    if text.is_empty() {
        return;
    }
    let emphasis = Emphasis::from_stack(stack);
    let styled = span(std::mem::take(text))
        .font(emphasis.font(font.iced_font, mono))
        .size(size)
        .color(color)
        .underline(emphasis.underline);
    spans.push(styled);
}

fn apply_tag(tag: &str, stack: &mut Vec<String>) {
    if let Some(name) = tag.strip_prefix('/') {
        let name = name.trim().to_ascii_lowercase();
        if let Some(pos) = stack.iter().rposition(|t| *t == name) {
            stack.remove(pos);
        }
    } else {
        // Keep only the element name, dropping any attributes.
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !name.is_empty() && !tag.ends_with('/') {
            stack.push(name);
        }
    }
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
            descent_px: 0.0,
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
