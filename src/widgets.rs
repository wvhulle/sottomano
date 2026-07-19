//! Shared rendering primitives (odin's `ReusableComponent` equivalent): the
//! visual vocabulary every leaf widget and concrete widget is drawn with — a
//! gauge fill, a state indicator, the edge-anchored layout it sits in, and the
//! source-backed leaf rendering that combines them. Not a widget itself; this
//! is what widgets are drawn with, so it lives beside `bar_layout` rather than
//! inside it. Each function takes exactly the data it needs (`&Config`,
//! `Position`) rather than a shared render context.

pub mod activity_line;
pub mod icon_gauge;
pub mod pager_slot;
pub mod text;

pub use activity_line::activity_line;
pub use icon_gauge::source_reading;
pub use pager_slot::pager_slot;
pub use text::bar_text;
