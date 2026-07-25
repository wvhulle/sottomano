//! Shared rendering primitives (odin's `ReusableComponent` equivalent): the
//! visual vocabulary every leaf widget and concrete widget is drawn with — a
//! gauge fill, a state indicator, the edge-anchored layout it sits in, and the
//! source-backed leaf rendering that combines them. Not a widget itself; this
//! is what widgets are drawn with, so it lives beside `bar_layout` rather than
//! inside it. Each function takes exactly the data it needs (`&ResolvedConfig`,
//! `Position`) rather than a shared render context.

pub mod activity_line;
pub mod icon;
pub mod icon_gauge;
pub mod idle;
pub mod inline_slider;
pub mod menu_group;
pub mod pager_slot;
pub mod slider_control;
pub mod reveal;
pub mod text;
