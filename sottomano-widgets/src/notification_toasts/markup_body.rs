use iced::{
    Element, Font, Length,
    alignment::{Horizontal, Vertical},
    border,
    font::{Style as FontStyle, Weight},
    widget::{
        Column, container, rich_text, span,
        text::{Span, Wrapping},
    },
};
use quick_xml::{Reader, escape::resolve_xml_entity, events::Event, name::LocalName};
use sottomano_toml_config::theme::font::ResolvedFont;

use crate::widgets::{
    content_sized_table::content_sized_grid,
    text::{LINE_HEIGHT, font_faces_of},
};

pub struct MarkupBody<'a> {
    markup: &'a str,
    font: &'a ResolvedFont,
    size: f32,
    monospace: Font,
    block_spacing: f32,
    column_gap: f32,
    corner_radius: f32,
}

impl<'a> MarkupBody<'a> {
    #[must_use]
    pub fn new(markup: &'a str, font: &'a ResolvedFont) -> Self {
        Self {
            markup,
            font,
            size: font.text_size_px,
            monospace: Font::MONOSPACE,
            block_spacing: 0.0,
            column_gap: 0.0,
            corner_radius: 0.0,
        }
    }

    #[must_use]
    pub fn size(self, size: f32) -> Self {
        Self { size, ..self }
    }

    #[must_use]
    pub fn monospace(self, monospace: Font) -> Self {
        Self { monospace, ..self }
    }

    #[must_use]
    pub fn column_gap(self, column_gap: f32) -> Self {
        Self { column_gap, ..self }
    }

    #[must_use]
    pub fn block_spacing(self, block_spacing: f32) -> Self {
        Self {
            block_spacing,
            ..self
        }
    }

    #[must_use]
    pub fn corner_radius(self, corner_radius: f32) -> Self {
        Self {
            corner_radius,
            ..self
        }
    }

    #[must_use]
    pub fn view<Message: 'static>(self) -> Element<'static, Message> {
        let blocks = parse_blocks(self.markup, self.size, self.font, self.monospace);
        Column::with_children(blocks.into_iter().map(|block| match block {
            Block::Paragraph {
                spans,
                preformatted,
            } => self.paragraph(spans, preformatted),
            Block::Rows(rows) => self.rows_table(rows),
        }))
        .spacing(self.block_spacing)
        .width(Length::Fill)
        .into()
    }

    fn paragraph<Message: 'static>(
        &self,
        spans: Vec<Span<'static>>,
        preformatted: bool,
    ) -> Element<'static, Message> {
        let flowing = rich_text(spans)
            .line_height(LINE_HEIGHT)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill);
        if !preformatted {
            return flowing.into();
        }
        let corner_radius = self.corner_radius;
        container(flowing)
            .padding(self.block_spacing)
            .width(Length::Fill)
            .style(move |theme| {
                container::rounded_box(theme).border(border::rounded(corner_radius))
            })
            .into()
    }

    fn rows_table<Message: 'static>(&self, rows: Vec<Row<'static>>) -> Element<'static, Message> {
        let upright_face = font_faces_of(self.font).regular;
        content_sized_grid(
            vec![Horizontal::Right, Horizontal::Left],
            rows.into_iter().map(|row| {
                let italic_key: Vec<_> = row
                    .key
                    .into_iter()
                    .map(|styled| {
                        let face = styled.font.unwrap_or(upright_face);
                        styled.font(Font {
                            style: FontStyle::Italic,
                            ..face
                        })
                    })
                    .collect();
                vec![
                    rich_text(italic_key)
                        .line_height(LINE_HEIGHT)
                        .wrapping(Wrapping::None)
                        .into(),
                    rich_text(row.value)
                        .line_height(LINE_HEIGHT)
                        .wrapping(Wrapping::WordOrGlyph)
                        .width(Length::Fill)
                        .into(),
                ]
            }),
        )
        .column_gap(self.column_gap)
        .row_gap(self.block_spacing)
        .cells_aligned_y(Vertical::Top)
        .into()
    }
}

impl<Message: 'static> From<MarkupBody<'_>> for Element<'static, Message> {
    fn from(body: MarkupBody<'_>) -> Self {
        body.view()
    }
}

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
struct Row<'a> {
    key: Vec<Span<'a>>,
    value: Vec<Span<'a>>,
}

enum Block<'a> {
    Paragraph {
        spans: Vec<Span<'a>>,
        preformatted: bool,
    },
    Rows(Vec<Row<'a>>),
}

struct BlockBuilder<'a> {
    blocks: Vec<Block<'a>>,
}

impl<'a> BlockBuilder<'a> {
    fn open_rows(&mut self) {
        self.blocks.push(Block::Rows(Vec::new()));
    }

    fn open_row(&mut self) {
        if let Some(Block::Rows(rows)) = self.blocks.last_mut() {
            rows.push(Row {
                key: Vec::new(),
                value: Vec::new(),
            });
        }
    }

    fn push_span(&mut self, styled: Span<'a>, stack: &[String]) {
        let inside = |tag: &str| stack.iter().any(|open| open == tag);
        if inside("dl") {
            if let Some(Block::Rows(rows)) = self.blocks.last_mut()
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
            Some(Block::Paragraph {
                spans,
                preformatted: open,
            }) if *open == preformatted => spans.push(styled),
            _ => self.blocks.push(Block::Paragraph {
                spans: vec![styled],
                preformatted,
            }),
        }
    }
}

fn parse_blocks<'a>(markup: &str, size: f32, font: &ResolvedFont, mono: Font) -> Vec<Block<'a>> {
    let mut reader = Reader::from_str(markup);
    let config = reader.config_mut();
    config.allow_dangling_amp = true;
    config.allow_unmatched_ends = true;
    config.check_end_names = false;

    let mut stack: Vec<String> = Vec::new();
    let mut builder = BlockBuilder { blocks: Vec::new() };
    let styled = |text: String, stack: &[String]| {
        let emphasis = Emphasis::from_stack(stack, font_faces_of(font).regular, mono);
        span(text)
            .font(emphasis.font)
            .size(size)
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
                let unparsed_from =
                    usize::try_from(reader.error_position()).unwrap_or(markup.len());
                builder.push_span(styled(markup[unparsed_from..].to_owned(), &stack), &stack);
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

    use sottomano_toml_config::theme::font::FontRole;

    use super::*;

    fn blocks(markup: &str) -> Vec<Block<'static>> {
        let font = ResolvedFont {
            role: FontRole::Text,
            family: String::from("sans-serif"),
            text_size_px: 14.0,
            small_text_size_px: 12.0,
            icon_size_px: 14.0,
            data: Arc::from([].as_slice()),
        };
        parse_blocks(markup, font.text_size_px, &font, Font::MONOSPACE)
    }

    fn joined(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.text.to_string()).collect()
    }

    fn render(markup: &str) -> String {
        blocks(markup)
            .iter()
            .map(|block| match block {
                Block::Paragraph { spans, .. } => joined(spans),
                Block::Rows(rows) => rows
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
        let Some(Block::Rows(rows)) = parsed.first() else {
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
