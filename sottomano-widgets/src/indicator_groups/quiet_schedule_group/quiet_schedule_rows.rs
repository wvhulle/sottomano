use chrono::{Locale, Utc, Weekday, WeekdaySet};
use corredo::value_controls::weekday_name;
use iced::Function;
use sottomano_toml_config::Config;

use super::{
    QuietScheduleGroup, QuietScheduleMessage, QuietSlider,
    quiet_window::{QuietPhase, QuietScheduleAdherence},
    weekday_picker::{WeekdayPicker, WeekdayPickerMessages, weekday_press_actions},
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{GroupEvent, IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

const WORKING_DAY_PRESSES: SwitchPresses = SwitchPresses {
    turning_on: "count as a working day",
    turning_off: "count as a day off",
};

impl QuietScheduleGroup {
    fn follow_schedule_toggle(&self) -> IconToggle {
        IconToggle::new(
            "Follow schedule",
            Phosphor::Repeat(Weight::Regular),
            self.schedule_adherence,
            SwitchPresses {
                turning_on: "start and end do not disturb on the daily schedule",
                turning_off: "stop following the daily quiet schedule",
            },
        )
    }

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

    fn working_days_hover_card(&self, locale: Locale) -> Vec<Section> {
        let working_days = self.working_days;
        vec![Section::new(
            "Schedule",
            dictionary_table(WeekdaySet::ALL.iter(Weekday::Mon).map(|day| {
                let day_kind = if working_days.contains(day) {
                    "working day"
                } else {
                    "day off"
                };
                LabeledValue::new(weekday_name(day, locale), day_kind)
            })),
        )]
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        let slider = |slider| {
            GroupIndicator::slider(
                self.slider(slider),
                config,
                QuietScheduleMessage::Slider.with(slider),
            )
        };
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
                GroupIndicator::toggle(
                    self.follow_schedule_toggle(),
                    QuietScheduleMessage::ToggleFollowsSchedule,
                ),
                slider(QuietSlider::Start),
                slider(QuietSlider::End),
                slider(QuietSlider::ScreenOff),
                GroupIndicator::composed(
                    "Working days",
                    IndicatorIcon::Absent,
                    IndicatorValue::drawn(move |reveal| {
                        WeekdayPicker::new(
                            config,
                            self.working_days,
                            reveal,
                            WeekdayPickerMessages {
                                on_toggle: |day| {
                                    GroupEvent::from(QuietScheduleMessage::ToggleWorkingDay(day))
                                },
                            },
                        )
                        .into()
                    }),
                    Box::new(move |_| self.working_days_hover_card(config.calendar.locale)),
                )
                .with_button_actions(weekday_press_actions(
                    self.working_days,
                    WORKING_DAY_PRESSES,
                    config.calendar.locale,
                )),
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
