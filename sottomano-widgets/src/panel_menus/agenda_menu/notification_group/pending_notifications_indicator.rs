use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::agenda_menu::quiet_schedule_group::quiet_window::QuietPhase,
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

#[derive(Clone, Copy)]
pub struct NotificationStatus {
    pub quiet_phase: QuietPhase,
    pub live: usize,
    pub held: usize,
}

impl NotificationStatus {
    pub(super) fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let glyph = match self.quiet_phase {
            QuietPhase::Active => Phosphor::ChatCircleSlash(Weight::Regular).into(),
            QuietPhase::Inactive => Phosphor::ChatCircle(Weight::Regular).into(),
        };
        let state = match self.quiet_phase {
            QuietPhase::Active => IndicatorState::Off,
            QuietPhase::Inactive if self.live > 0 => IndicatorState::On,
            QuietPhase::Inactive => IndicatorState::Idle,
        };
        let badge = match self.live + self.held {
            0 => ReadoutBadge::Unbadged,
            count => ReadoutBadge::Counted(count),
        };
        GroupIndicator::glyph_only("Pending notifications", glyph, state)
            .with_readout(IndicatorReadout {
                label: ReadoutLabel::Unlabeled,
                gauge: ReadoutGauge::Ungauged,
                badge,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(self) -> Vec<Section> {
        if self.held > 0 {
            vec![Section::new(
                "Counts",
                dictionary_table([LabeledValue::new(
                    "Held until do not disturb ends",
                    Quantity::count(self.held),
                )]),
            )]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NotificationStatus, QuietPhase, ReadoutBadge};

    #[test]
    fn the_headline_counts_live_and_held_notifications() {
        let pending = NotificationStatus {
            quiet_phase: QuietPhase::Inactive,
            live: 2,
            held: 1,
        };
        assert_eq!(
            pending
                .indicator()
                .shown_readout()
                .expect("a readout indicator")
                .badge,
            ReadoutBadge::Counted(3)
        );
    }

    #[test]
    fn the_headline_has_no_count_without_notifications() {
        let quiet = NotificationStatus {
            quiet_phase: QuietPhase::Inactive,
            live: 0,
            held: 0,
        };
        assert_eq!(
            quiet
                .indicator()
                .shown_readout()
                .expect("a readout indicator")
                .badge,
            ReadoutBadge::Unbadged
        );
    }
}
