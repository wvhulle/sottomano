//! odin's `ReusableComponent/MenuGroup`: an expandable in-place group. A
//! persistent handle sits on the bar; toggling unfolds a hidden content row to
//! its right — the content fades in and the horizontal space grows from zero,
//! while an edge hairline brightens to mark the group active. One eased scalar
//! (`reveal`, 0..1) drives all three. Concrete groups own their data plus one
//! `MenuGroup` and supply a handle and a content row.
//!
//! The group also owns the shared interaction: a single [`MenuMessage`]
//! (toggle / cursor-enter / cursor-exit / close) instead of each concrete group
//! re-declaring those, and [`view`](MenuGroup::view) wraps the composed element
//! in the hover mouse-area itself. The expand animation is self-driving — the
//! [`redraw_while`] wrapper re-requests frames while the curve runs — so nothing
//! about ticking leaks up to the application.

use iced::{
    Alignment, Animation, Element, Padding,
    animation::Easing,
    time::{Duration, Instant},
    widget::{Row, container, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    layer_surface::Position,
    widgets::{
        activity_line::activity_line, animate::redraw_while, pager_slot::PagerSlot,
        reveal::reveal_x,
    },
};

/// The interactions every expandable group shares. One variant per concrete
/// interaction — no boolean-flag payload; `CursorEntered`/`CursorExited` mirror
/// the `mouse_area` events they originate from.
#[derive(Debug, Clone, Copy)]
pub enum MenuMessage {
    Toggle,
    CursorEntered,
    CursorExited,
    Close,
}

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

    /// Apply a shared interaction. Returns `true` only on the open→closed edge
    /// (this message collapsed an open menu), so callers can tear down
    /// dependent state exactly once — never on a mere hover.
    pub fn update(&mut self, msg: MenuMessage) -> bool {
        let was_open = self.is_open();
        match msg {
            MenuMessage::Toggle => self.expand.go_mut(!self.is_open(), Instant::now()),
            MenuMessage::CursorEntered => self.hovered = true,
            MenuMessage::CursorExited => self.hovered = false,
            MenuMessage::Close => self.expand.go_mut(false, Instant::now()),
        }
        was_open && !self.is_open()
    }

    /// Whether the group is (targeting) open — for closing dependent state when
    /// it collapses.
    pub fn is_open(&self) -> bool {
        self.expand.value()
    }

    /// The eased reveal fraction sampled at the current instant. Because the
    /// animation is a pure curve, this samples `Instant::now()` directly; the
    /// [`redraw_while`] wrapper keeps frames coming so successive samples
    /// advance the transition.
    pub fn reveal(&self) -> f32 {
        self.expand.interpolate(0.0, 1.0, Instant::now())
    }

    /// Compose the group: `handle` (always shown) beside the revealed `content`,
    /// in the shared pager slot with an edge hairline, wrapped in the hover
    /// mouse-area (its enter/exit mapped through `to_message`) and the
    /// self-driving redraw wrapper. The handle owns the toggle press, so the
    /// caller wires `.on_press(to_message(MenuMessage::Toggle))` onto it.
    ///
    /// `reveal` is passed in so the caller can fade its own `content` colours by
    /// the same factor it computed once via [`reveal`](Self::reveal).
    pub fn view<'a, Message: 'a + Clone, F>(
        &self,
        config: &ResolvedConfig,
        position: Position,
        reveal: f32,
        handle: Element<'a, Message>,
        content: Element<'a, Message>,
        to_message: F,
    ) -> Element<'a, Message>
    where
        F: Fn(MenuMessage) -> Message,
    {
        let inner = Row::with_children([handle, revealed_content(config, reveal, content)])
            .align_y(Alignment::Center);
        let slot =
            PagerSlot::new(inner.into(), edge_indicator(config, self.hovered, reveal)).view(position);
        let hoverable = mouse_area(slot)
            .on_enter(to_message(MenuMessage::CursorEntered))
            .on_exit(to_message(MenuMessage::CursorExited));
        redraw_while(self.expand.clone(), hoverable)
    }
}

/// The content row, gap-padded and horizontally scaled by `reveal` so the
/// handle→content gap grows as the group unfolds (odin's `anim * (gap + content)`).
fn revealed_content<'a, Message: 'a>(
    config: &ResolvedConfig,
    reveal: f32,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let gap = config.layout.horizontal_widget_spacing_px;
    reveal_x(container(content).padding(Padding::ZERO.left(gap)), reveal)
}

/// The inner-edge hairline marking the group active; brightens toward the focus
/// colour as the group is hovered / revealed.
fn edge_indicator<'a, Message: 'a>(
    config: &ResolvedConfig,
    hovered: bool,
    reveal: f32,
) -> Element<'a, Message> {
    let base = if hovered { 0.5 } else { 0.2 };
    activity_line(
        config.palette.focus,
        base + (1.0 - base) * reveal,
        config.layout.focus_underline_thickness_px,
    )
}
