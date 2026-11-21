use iced::alignment::Horizontal;
use sottomano_desktop_sources::ai_usage_spend::llama_server_slots::{
    LlamaSlot, SlotAssignment, SlotTask, SlotWork,
};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        value_cells::ValueCell,
    },
};

fn context_fill(task: &SlotTask, context: u32) -> Ratio {
    Ratio::new::<ratio>(f64::from(task.tokens()) / f64::from(context))
}

#[derive(Clone, Copy)]
pub struct ContextFill<'a> {
    pub slots: &'a [LlamaSlot],
    pub fullest: Ratio,
    pub server: &'a str,
}

impl<'a> ContextFill<'a> {
    pub fn fullest_busy_slot(slots: &'a [LlamaSlot], server: &'a str) -> Option<Self> {
        let fullest = slots
            .iter()
            .filter(|slot| slot.work == SlotWork::Processing)
            .filter_map(|slot| match slot.task {
                SlotAssignment::Assigned(task) => Some(context_fill(&task, slot.context)),
                SlotAssignment::Unassigned => None,
            })
            .max_by(|left, right| left.value.total_cmp(&right.value))?;
        Some(Self {
            slots,
            fullest,
            server,
        })
    }
}

fn slot_record(slot: &LlamaSlot) -> TableRecord<3> {
    TableRecord::with_cells(
        slot.id.to_string(),
        [
            match slot.work {
                SlotWork::Processing => "processing",
                SlotWork::Idle => "idle",
            }
            .into(),
            match slot.task {
                SlotAssignment::Assigned(task) => {
                    format!("{} / {}", task.tokens(), slot.context).into()
                }
                SlotAssignment::Unassigned => ValueCell::Blank,
            },
            match slot.task {
                SlotAssignment::Assigned(task) => {
                    Quantity::share(context_fill(&task, slot.context)).into()
                }
                SlotAssignment::Unassigned => ValueCell::Blank,
            },
        ],
    )
}

impl<'a> ContextFill<'a> {
    pub fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            format!("{} context fill", self.server),
            Phosphor::Scroll(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged {
                fill: self.fullest,
                live: self.fullest,
            },
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Slots",
            headed_table(
                [
                    ColumnHeading::titled("State").align_x(Horizontal::Left),
                    ColumnHeading::titled("Tokens"),
                    ColumnHeading::titled("Fill"),
                ],
                self.slots.iter().map(slot_record),
            ),
        )]
    }
}
