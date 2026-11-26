pub mod weekday_picker;

use chrono::{Locale, Weekday, WeekdaySet};
use corredo::value_controls::weekday_name;
use sottomano_toml_config::Config;

use self::weekday_picker::{WeekdayPicker, WeekdayPickerMessages, weekday_press_actions};
use super::{QuietScheduleGroup, QuietScheduleMessage};
use crate::{
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_group_content::GroupEvent,
        toggle_control::SwitchPresses,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

const WORKING_DAY_PRESSES: SwitchPresses = SwitchPresses {
    turning_on: "count as a working day",
    turning_off: "count as a day off",
};

impl QuietScheduleGroup {
    pub(super) fn working_days_row<'view>(
        &'view self,
        config: &'view Config,
    ) -> GroupIndicator<'view> {
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
        ))
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
}
