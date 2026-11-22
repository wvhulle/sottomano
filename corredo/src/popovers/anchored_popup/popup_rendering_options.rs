use iced::{Element, Point, Rectangle, advanced::layout::Node};

use crate::popovers::CascadeOrientation;

#[derive(Debug, Clone)]
pub struct PopupRegions {
    pub surface_rows: Vec<Rectangle>,
    pub popup_bounds: Rectangle,
    pub bounds: Rectangle,
    pub pointer_input: PopupPointerInput,
    pub outside_press_capture: OutsidePressCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupPointerInput {
    Accepted,
    PassedThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutsidePressCapture {
    Captured,
    PassedThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupFraming {
    WholeContent,
    LeadingSurface(CascadeOrientation),
}

impl PopupFraming {
    pub(super) fn framed_bounds(self, node: &Node) -> Rectangle {
        let whole = Rectangle::new(Point::ORIGIN, node.size());
        match self {
            PopupFraming::WholeContent => whole,
            PopupFraming::LeadingSurface(_) => node.children().first().map_or(whole, Node::bounds),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupArrival {
    Incoming,
    Outgoing,
}

pub struct PopupRendering<'view, Message> {
    pub(super) element: Element<'view, Message>,
    pub arrival: PopupArrival,
    pub framing: PopupFraming,
}

impl<'view, Message> PopupRendering<'view, Message> {
    #[must_use]
    pub fn incoming(element: Element<'view, Message>) -> Self {
        Self {
            element,
            arrival: PopupArrival::Incoming,
            framing: PopupFraming::WholeContent,
        }
    }

    #[must_use]
    pub fn outgoing(element: Element<'view, Message>) -> Self {
        Self {
            element,
            arrival: PopupArrival::Outgoing,
            framing: PopupFraming::WholeContent,
        }
    }

    #[must_use]
    pub fn framing(self, framing: PopupFraming) -> Self {
        Self { framing, ..self }
    }
}

pub enum OutsidePress<Message> {
    PassesThrough,
    Dismisses {
        dismissal: Message,
        reachable_below: Vec<Rectangle>,
    },
}
