use std::path::Path;

use corredo::{
    changing_text::TransitioningText, press_actions::flash_on_press_text, theming::LINE_HEIGHT,
};
use iced::{
    Color, Element, Theme,
    mouse::Interaction,
    widget::{
        container, rich_text, span,
        text::{Span, Style as TextStyle, Wrapping},
    },
};

use super::{CellTypeFace, ValueCell, ValueCellView};
use crate::widgets::text::{panel_text, resolved_text_face};

impl<Message: 'static> ValueCellView<Message> {
    pub(super) fn path(
        &self,
        path: &Path,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        let text = path.display().to_string();
        let unwrapped = panel_text(&self.prose_font, text.clone())
            .font(self.styled_face(&self.prose_font))
            .style(move |theme| TextStyle {
                color: Some(color(theme)),
            })
            .wrapping(Wrapping::None);
        let value = TransitioningText::new(
            unwrapped,
            text.clone(),
            self.transition,
            resolved_text_face(&self.prose_font),
            color,
        )
        .identified_by(row);
        Element::from(
            flash_on_press_text(value, text, self.press_messages.copy_text)
                .accent(self.flash)
                .hold(self.copy_flash)
                .interaction(Interaction::Copy),
        )
    }

    pub(super) fn underlined_label(
        &self,
        label: String,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        link_cell: &ValueCell,
    ) -> Element<'static, Message> {
        let underlined_label: Span<'static> = span(label.clone())
            .font(self.styled_face(&self.prose_font))
            .underline(true);
        let underlined = container(
            rich_text([underlined_label])
                .style(move |theme| TextStyle {
                    color: Some(color(theme)),
                })
                .size(self.prose_font.text_size_px)
                .line_height(LINE_HEIGHT)
                .wrapping(self.wrapping),
        )
        .max_width(self.max_width);
        TransitioningText::new(
            underlined,
            label,
            self.transition,
            resolved_text_face(&self.prose_font),
            color,
        )
        .identified_by(link_cell)
        .into()
    }

    pub(super) fn openable(
        &self,
        label: Element<'static, Message>,
        url: String,
    ) -> Element<'static, Message> {
        Element::from(
            flash_on_press_text(label, url, self.press_messages.open_url)
                .accent(self.link)
                .hold(self.copy_flash)
                .interaction(Interaction::Pointer),
        )
    }

    pub(super) fn copyable_figures(
        &self,
        text: String,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        self.copyable_text(text, CellTypeFace::Figures, color, row)
    }

    pub(super) fn copyable_text(
        &self,
        text: String,
        face: CellTypeFace,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        let value = self.text(text.clone(), face, color, row);
        if text.trim().is_empty() {
            return value;
        }
        Element::from(
            flash_on_press_text(value, text, self.press_messages.copy_text)
                .accent(self.flash)
                .hold(self.copy_flash)
                .interaction(Interaction::Copy),
        )
    }
}
