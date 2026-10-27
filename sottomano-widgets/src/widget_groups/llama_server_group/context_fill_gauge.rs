use iced::alignment::Horizontal;
use sottomano_sources::ai_usage_spend::llama_server_slots::{LlamaSlot, SlotTask};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
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
                .map(|task| Quantity::share(context_fill(&task, slot.context)).into()),
        ],
    )
}

impl BarWidget for ContextFill<'_> {
    fn name(&self) -> &'static str {
        "Context fill"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Scroll(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Scroll(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.fullest)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::titled("State").align_x(Horizontal::Left),
                ColumnHeading::titled("Tokens"),
                ColumnHeading::titled("Fill"),
            ],
            self.slots.iter().map(slot_record),
        )
        .titled("Slots")])
    }
}
