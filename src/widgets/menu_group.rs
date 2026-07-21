//! odin's `ReusableComponent/MenuGroup`: an expandable in-place group. A
//! persistent handle sits on the bar; toggling unfolds a hidden content row to
//! its right — the content fades in and the horizontal space grows from zero,
//! while an edge hairline brightens to mark the group active. One eased scalar
//! (`reveal`, 0..1) drives all three. Concrete groups (system, and later
//! brightness/media/…) own their data plus one `MenuGroup` and supply a handle
//! and a content row.

use iced::{
    Alignment, Animation, Element, Padding,
    animation::Easing,
    time::{Duration, Instant},
    widget::{Row, container},
};

use crate::{
    bar_program::Message,
    layer_surface::Position,
    user_config::Config,
    widgets::{activity_line::activity_line, pager_slot::pager_slot, reveal::reveal_x},
};

pub struct MenuGroup {
    expand: Animation<bool>,
    hovered: bool,
}

impl MenuGroup {
    pub fn new(expand_ms: u64) -> Self {
        Self {
            expand: Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(Duration::from_millis(expand_ms)),
            hovered: false,
        }
    }

    pub fn toggle(&mut self, now: Instant) {
        self.expand.go_mut(!self.expand.value(), now);
    }

    pub fn close(&mut self, now: Instant) {
        self.expand.go_mut(false, now);
    }

    pub fn set_hover(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.expand.is_animating(now)
    }

    pub fn reveal(&self, now: Instant) -> f32 {
        self.expand.interpolate(0.0, 1.0, now)
    }

    /// Compose the group's visuals: `handle` (always shown) beside the revealed
    /// `content`, under the shared `pager_slot` with an edge hairline. `reveal`
    /// is passed in so the caller computes it once and fades `content`'s
    /// colours by the same factor. The caller wraps the result in its own
    /// `mouse_area` (routing `toggle`/`set_hover`), so the interaction
    /// lives with the group's messages rather than being threaded through
    /// here.
    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
        reveal: f32,
        handle: Element<'a, Message>,
        content: Element<'a, Message>,
    ) -> Element<'a, Message> {
        // Pad the content on its left so the handle→content gap grows with the
        // reveal too (odin's `anim * (gap + content)`).
        let gap = config.layout.horizontal_widget_spacing;
        let expanded = reveal_x(container(content).padding(Padding::ZERO.left(gap)), reveal);

        let inner = Row::with_children([handle, expanded]).align_y(Alignment::Center);

        let base = if self.hovered { 0.5 } else { 0.2 };
        let indicator = activity_line(
            config.palette.focus(),
            base + (1.0 - base) * reveal,
            config.layout.focus_underline_thickness,
        );

        pager_slot(position, inner.into(), indicator, None)
    }
}
