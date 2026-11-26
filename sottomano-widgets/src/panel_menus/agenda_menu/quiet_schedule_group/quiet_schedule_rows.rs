use chrono::Utc;
use sottomano_toml_config::Config;

use super::{
    QuietScheduleGroup, QuietScheduleMessage, QuietSlider,
    quiet_window::{QuietPhase, QuietScheduleAdherence},
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

impl QuietScheduleGroup {
    #[must_use]
    pub fn do_not_disturb_hover_card(&self, clock: Clock) -> Vec<Section> {
        let now = clock.local();
        let manual = self
            .manual_at(now)
            .map(|_| LabeledValue::new("Set", "by hand, until the schedule next changes"));
        let upcoming = self.next_change(now).map(|at| {
            let label = match self.phase {
                QuietPhase::Active => "Quiet until",
                QuietPhase::Inactive => "Quiet from",
            };
            [
                LabeledValue::new(
                    label,
                    ValueCell::Time {
                        at: at.with_timezone(&Utc),
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                ),
                LabeledValue::new("In", ValueCell::duration(at - now)),
            ]
        });
        vec![Section::new(
            "Next change",
            dictionary_table(manual.into_iter().chain(upcoming.into_iter().flatten())),
        )]
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::QuietSchedule,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::QuietSchedule.title(),
                self.face_icon().into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            QuietScheduleMessage::Expansion,
            IndicatorListing::Listed(Vec::from([
                self.follow_schedule_row(),
                self.quiet_slider_row(config, QuietSlider::Start),
                self.quiet_slider_row(config, QuietSlider::End),
                self.quiet_slider_row(config, QuietSlider::ScreenOff),
                self.working_days_row(config),
            ])),
        )
    }

    fn face_icon(&self) -> Phosphor {
        match self.schedule_adherence {
            QuietScheduleAdherence::Followed => Phosphor::ClockUser(Weight::Fill),
            QuietScheduleAdherence::Ignored => Phosphor::ClockUser(Weight::Regular),
        }
    }
}
