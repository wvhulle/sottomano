use corredo::{
    frame_style::FrameStyle,
    pointer_drag::{
        DragEvent, DragOutcome, DragView, PointerDrag, SlotPress, advance_within_one_list,
        draggable_slot::draggable_slot,
        pointer_events,
        slot_pager::{PagerEdge, slot_pager},
    },
    theme_colors::primary_color,
};
use iced::{
    Element, Length, Subscription,
    time::Instant,
    widget::{container, text},
};

type SlotDragEvent = DragEvent<u32, (), u32>;

#[derive(Debug, Clone, Copy)]
enum PagerMessage {
    Dragged(SlotDragEvent),
}

struct PagerDemo {
    slots: Vec<u32>,
    selected: u32,
    drag: PointerDrag<u32, u32>,
}

impl Default for PagerDemo {
    fn default() -> Self {
        Self {
            slots: vec![1, 2, 3, 4, 5],
            selected: 1,
            drag: PointerDrag::Idle,
        }
    }
}

impl PagerDemo {
    #[allow(clippy::disallowed_methods)]
    fn update(&mut self, message: PagerMessage) {
        let PagerMessage::Dragged(event) = message;
        let transition = advance_within_one_list(self.drag, event, Instant::now());
        self.drag = transition.drag;
        match transition.outcome {
            DragOutcome::Clicked(clicked) => self.selected = clicked,
            DragOutcome::Dropped(dropped) => {
                if let Some(target) = dropped.reordered_to() {
                    let moved = self.slots.remove(dropped.from);
                    self.slots.insert(target.min(self.slots.len()), moved);
                }
            }
            DragOutcome::Unchanged | DragOutcome::Started | DragOutcome::HoveringForeign(_) => {}
        }
    }

    fn view(&self) -> Element<'_, PagerMessage> {
        let drag_view = DragView::new(&self.drag, FrameStyle::default());
        let shown = drag_view.previewed(&self.slots, |slot| *slot);
        let slots = shown.into_iter().enumerate().map(|(position, slot)| {
            let press = SlotPress {
                item: *slot,
                position,
                click: *slot,
            };
            let label = container(text(format!("Slot {slot}"))).padding(12);
            let draggable: Element<'_, PagerMessage> =
                draggable_slot(label, press, true, &drag_view, PagerMessage::Dragged).into();
            (*slot, draggable)
        });
        container(
            slot_pager(slots, PagerEdge::Top)
                .spacing(4.0)
                .highlight(Some(self.selected), primary_color)
                .draggable((), &drag_view, PagerMessage::Dragged),
        )
        .center(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<PagerMessage> {
        pointer_events(!self.drag.is_idle()).map(PagerMessage::Dragged)
    }
}

fn main() -> iced::Result {
    iced::application(PagerDemo::default, PagerDemo::update, PagerDemo::view)
        .subscription(PagerDemo::subscription)
        .run()
}
