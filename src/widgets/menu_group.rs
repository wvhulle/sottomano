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
    config::resolved::ResolvedConfig,
    layer_surface::Position,
    widgets::{activity_line::activity_line, pager_slot::PagerSlot, reveal::reveal_x},
};

pub struct MenuGroup {
    expand: Animation<bool>,
    hovered: bool,
    /// Timestamp the pure view samples the expand curve at; refreshed by every
    /// state change and by [`tick`](Self::tick) on each animation frame.
    last_tick: Instant,
}

impl MenuGroup {
    pub fn new(expand_ms: u64) -> Self {
        Self {
            expand: Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(Duration::from_millis(expand_ms)),
            hovered: false,
            last_tick: Instant::now(),
        }
    }

    pub fn toggle(&mut self) {
        self.last_tick = Instant::now();
        self.expand.go_mut(!self.expand.value(), self.last_tick);
    }

    pub fn close(&mut self) {
        self.last_tick = Instant::now();
        self.expand.go_mut(false, self.last_tick);
    }

    pub fn set_hover(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub fn tick(&mut self, now: Instant) {
        self.last_tick = now;
    }

    pub fn is_animating(&self) -> bool {
        self.expand.is_animating(Instant::now())
    }

    /// Whether the group is (targeting) open — for closing dependent state when
    /// it collapses.
    pub fn is_open(&self) -> bool {
        self.expand.value()
    }

    pub fn reveal(&self) -> f32 {
        self.expand.interpolate(0.0, 1.0, self.last_tick)
    }

    /// Compose the group's visuals: `handle` (always shown) beside the revealed
    /// `content`, laid out in the shared pager slot with an edge hairline.
    /// `reveal` is passed in so the caller computes it once and fades
    /// `content`'s colours by the same factor. The caller wraps the result in
    /// its own mouse area (routing toggle and hover), so the interaction lives
    /// with the group's messages rather than being threaded through here.
    pub fn view<'a, Message: 'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
        reveal: f32,
        handle: Element<'a, Message>,
        content: Element<'a, Message>,
    ) -> Element<'a, Message> {
        // Pad the content on its left so the handle→content gap grows with the
        // reveal too (odin's `anim * (gap + content)`).
        let gap = config.layout.horizontal_widget_spacing_px;
        let expanded = reveal_x(container(content).padding(Padding::ZERO.left(gap)), reveal);

        let inner = Row::with_children([handle, expanded]).align_y(Alignment::Center);

        let base = if self.hovered { 0.5 } else { 0.2 };
        let indicator = activity_line(
            config.palette.focus,
            base + (1.0 - base) * reveal,
            config.layout.focus_underline_thickness_px,
        );

        PagerSlot::new(inner.into(), indicator).view(position)
    }
}
