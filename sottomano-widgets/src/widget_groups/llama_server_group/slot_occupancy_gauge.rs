use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::ai_usage_spend::llama_server_slots::LlamaSlot;
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct SlotOccupancy<'a> {
    pub slots: &'a [LlamaSlot],
    pub server: &'a str,
}

impl SlotOccupancy<'_> {
    fn busy(&self) -> usize {
        self.slots.iter().filter(|slot| slot.processing).count()
    }
}

impl GroupItemReading for SlotOccupancy<'_> {
    fn name(&self) -> Cow<'static, str> {
        format!("{} busy slots", self.server).into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Stack(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Stack(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{}/{}", self.busy(), self.slots.len()))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.busy() > 0 {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Idle
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<ratio>(
            self.busy() as f64 / self.slots.len() as f64,
        ))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Processing", Quantity::count(self.busy())),
            LabeledValue::new("Slots", Quantity::count(self.slots.len())),
        ])
    }
}
