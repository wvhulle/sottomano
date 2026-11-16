use chrono::{DateTime, Utc};
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, LAST_DAY, LAST_HOUR,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct ContextSwitchRate<'a> {
    pub accounting: Accounting<'a>,
}

impl ContextSwitchRate<'_> {
    fn per_hour(&self, now: DateTime<Utc>) -> usize {
        self.accounting
            .switches_within(LAST_HOUR.ending_at(now), now)
    }

    fn severity(&self, now: DateTime<Utc>) -> Severity {
        let config = self.accounting.config;
        let critical = usize::try_from(config.switches_per_hour_critical)
            .expect("usize holds a u32 on supported targets");
        let warning = usize::try_from(config.switches_per_hour_warning)
            .expect("usize holds a u32 on supported targets");
        match self.per_hour(now) {
            rate if rate > critical => Severity::Critical,
            rate if rate > warning => Severity::Warning,
            _ => Severity::Normal,
        }
    }

    pub fn sections(&self, now: DateTime<Utc>) -> [Section; 2] {
        let last_day = LAST_DAY.ending_at(now);
        let switches_last_day = self.accounting.switches_within(last_day, now);
        let active_last_day = self.accounting.screen_spent(last_day, now);
        let average_span = (switches_last_day > 0).then(|| {
            active_last_day
                / i32::try_from(switches_last_day)
                    .expect("a day holds fewer than i32::MAX focus switches")
        });
        let samples = Section::new(
            SectionTitle::captioned(
                "Window switches".to_owned(),
                "A switch is focusing another window, or the same window in another app or \
                 project; refocusing the same one doesn't count."
                    .to_owned(),
            ),
            dictionary_table(
                [
                    Some(
                        LabeledValue::new(
                            "Switches in the last hour",
                            Quantity::count(self.per_hour(now)),
                        )
                        .level(self.severity(now)),
                    ),
                    Some(LabeledValue::new(
                        "Switches in the last 24 hours",
                        Quantity::count(switches_last_day),
                    )),
                    average_span.map(|span| {
                        LabeledValue::new("Average time per window", ValueCell::duration(span))
                    }),
                    Some(LabeledValue::new(
                        "Active time in the last hour",
                        ValueCell::duration(
                            self.accounting.screen_spent(LAST_HOUR.ending_at(now), now),
                        ),
                    )),
                ]
                .into_iter()
                .flatten(),
            ),
        );
        let thresholds = Section::new(
            "Switch alerts",
            dictionary_table([
                LabeledValue::new(
                    "Warning above per hour",
                    Quantity::measured(
                        f64::from(self.accounting.config.switches_per_hour_warning),
                        DisplayUnit::PerHour,
                        0,
                    ),
                ),
                LabeledValue::new(
                    "Critical above per hour",
                    Quantity::measured(
                        f64::from(self.accounting.config.switches_per_hour_critical),
                        DisplayUnit::PerHour,
                        0,
                    ),
                ),
            ]),
        );
        [samples, thresholds]
    }
}

impl<'a> ContextSwitchRate<'a> {
    pub fn reading_row(self, clock: Clock) -> ReadingRow<'a> {
        ReadingRow {
            readout: IndicatorReadout {
                label: ReadoutLabel::Labeled(format!("{}/h", self.per_hour(clock.utc()))),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Context switches",
                Phosphor::Shuffle(Weight::Regular).into(),
                self.severity(clock.utc()).into(),
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.sections(clock.utc()).into_iter().collect::<Vec<_>>()
    }
}
