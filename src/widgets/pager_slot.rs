use iced::{Element, Length, alignment::Vertical, widget::container};

use crate::wayland_layers::surface::Position;

/// A pager/menu slot: `content` centred over the bar height, with an
/// `indicator` hairline pinned to the inner screen edge and optional `ticks`
/// (the per-window segments) pinned to the outer edge. The shared layout for
/// menu affordances, pager focus indicators, and the workspace window
/// indicators — odin's `Hairline` + `SlotHighlight` +
/// `WorkspaceWindowIndicators` composition.
///
/// A `Stack` (overlay, not an in-flow `Column`) so the bars sit *on* the bar's
/// own vertical padding rather than adding height — the slot stays the bar's
/// height, `content` centred, bars at the edges. The `Stack` sizes to its base
/// layer (`content`), so the Fill-width bars are constrained to the content
/// width and the slot is exactly as wide as its label. Inner edge = top on a
/// bottom bar, bottom on a top bar, so a bar always points into the screen.
pub struct PagerSlot<'a, Message> {
    content: Element<'a, Message>,
    indicator: Element<'a, Message>,
    ticks: Option<Element<'a, Message>>,
}

impl<'a, Message: 'a> PagerSlot<'a, Message> {
    /// A slot with its centred `content` and the inner-edge state hairline.
    pub fn new(content: Element<'a, Message>, indicator: Element<'a, Message>) -> Self {
        Self {
            content,
            indicator,
            ticks: None,
        }
    }

    /// Add the outer-edge per-window tick segments.
    #[must_use]
    pub fn ticks(mut self, ticks: Element<'a, Message>) -> Self {
        self.ticks = Some(ticks);
        self
    }

    pub fn view(self, position: Position) -> Element<'a, Message> {
        let (indicator_edge, ticks_edge) = match position {
            Position::Bottom => (Vertical::Top, Vertical::Bottom),
            Position::Top => (Vertical::Bottom, Vertical::Top),
        };
        // No horizontal padding: the state bars span the slot edge to edge (odin's
        // `PagerSlot` — "the slot is exactly its content: the state bars span it
        // edge to edge, so any padding here would widen the bars rather than
        // separate them"). Slot separation is the row `spacing`, never inset here.
        let edge_layer = |element, edge| {
            container(element)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(edge)
        };
        let base = container(self.content)
            .height(Length::Fill)
            .align_y(Vertical::Center);
        let mut layers: Vec<Element<'a, Message>> = vec![
            base.into(),
            edge_layer(self.indicator, indicator_edge).into(),
        ];
        if let Some(ticks) = self.ticks {
            layers.push(edge_layer(ticks, ticks_edge).into());
        }
        iced::widget::Stack::with_children(layers).into()
    }
}
