use iced::alignment::Horizontal;
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    data::source::llama_server::{LlamaSlot, SlotTask},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_text,
        phosphor::{Phosphor, Weight},
        tooltip_body::{ColumnHeading, Section, TableRecord, TooltipBody},
    },
};

fn context_fill(task: &SlotTask, context: u32) -> Ratio {
    Ratio::new::<ratio>(f64::from(task.tokens()) / f64::from(context))
}

pub struct ContextFill<'a> {
    pub slots: &'a [LlamaSlot],
    pub fullest: Ratio,
}

impl<'a> ContextFill<'a> {
    pub fn of_busy_slots(slots: &'a [LlamaSlot]) -> Option<Self> {
        let fullest = slots
            .iter()
            .filter(|slot| slot.processing)
            .filter_map(|slot| Some(context_fill(slot.task.as_ref()?, slot.context)))
            .max_by(|left, right| left.value.total_cmp(&right.value))?;
        Some(Self { slots, fullest })
    }
}

fn slot_record(slot: &LlamaSlot) -> TableRecord<3> {
    TableRecord::new(
        slot.id.to_string(),
        [
            Some(
                if slot.processing {
                    "processing"
                } else {
                    "idle"
                }
                .into(),
            ),
            slot.task
                .map(|task| format!("{} / {}", task.tokens(), slot.context).into()),
            slot.task
                .map(|task| percent_text(context_fill(&task, slot.context)).into()),
        ],
    )
}

impl BarWidget for ContextFill<'_> {
    fn name(&self) -> &'static str {
        "Context fill"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Scroll(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Scroll(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.fullest)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("State").align_x(Horizontal::Left),
                ColumnHeading::new("Tokens"),
                ColumnHeading::new("Fill"),
            ],
            self.slots.iter().map(slot_record),
        )
        .titled("Slots")])
    }
}
