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
