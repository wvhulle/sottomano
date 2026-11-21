use std::{cmp::Ordering, iter::once};

use chrono::{DateTime, Locale, TimeDelta, Utc};
use sottomano_desktop_sources::{
    ai_usage_spend::usage_quota::Quota,
    login_session_presence::focus_time_ledger::{
        Accounting, BudgetStanding, FocusUnit, LAST_DAY, UnitTotal,
    },
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_durations::duration_text,
        ongoing_toast::IndicatorAlerts,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct DailyTotalsGauge {
    pub computed_at: DateTime<Utc>,
    screen_spent: TimeDelta,
    screen_budget: BudgetStanding,
    units: Vec<UnitSpendRecord>,
}

impl DailyTotalsGauge {
    pub fn new(accounting: &Accounting<'_>, now: DateTime<Utc>) -> Self {
        let last_day = LAST_DAY.ending_at(now);
        Self {
            computed_at: now,
            screen_spent: accounting.screen_spent(last_day, now),
            screen_budget: accounting.screen_budget_standing(now),
            units: accounting
                .unit_totals(last_day, now)
                .into_iter()
                .map(|total| UnitSpendRecord {
                    budget: total.budget_standing(now),
                    total,
                })
                .collect(),
        }
    }

    fn worst(&self) -> Option<Quota> {
        self.quotas().into_iter().max_by(|left_quota, right_quota| {
            left_quota
                .used
                .partial_cmp(&right_quota.used)
                .unwrap_or(Ordering::Equal)
        })
    }

    pub fn sections(&self) -> Vec<Section> {
        let (apps, projects): (Vec<&UnitSpendRecord>, Vec<&UnitSpendRecord>) = self
            .units
            .iter()
            .partition(|record| matches!(record.total.unit, FocusUnit::App(_)));
        once(self.screen())
            .chain(UnitSpendRecord::section(&apps, "Applications"))
            .chain(UnitSpendRecord::section(&projects, "Projects"))
            .collect()
    }

    fn screen(&self) -> Section {
        let active = LabeledValue::new("Active (24h)", ValueCell::duration(self.screen_spent))
            .level(self.screen_budget.severity());
        let entries = match &self.screen_budget {
            BudgetStanding::Budgeted(status) => vec![
                active,
                LabeledValue::new("Daily budget", ValueCell::duration(status.budget)),
            ],
            BudgetStanding::Unbudgeted => vec![active],
        };
        Section::new(
            SectionTitle::captioned(
                "Screen".to_owned(),
                "Only active time in the last 24 hours counts, attributed to the focused window's \
                 app and project. A budget turns critical once that time exceeds it."
                    .to_owned(),
            ),
            dictionary_table(entries),
        )
    }
}

struct UnitSpendRecord {
    total: UnitTotal,
    budget: BudgetStanding,
}

impl UnitSpendRecord {
    fn section(records: &[&Self], title: &str) -> Option<Section> {
        if records.is_empty() {
            None
        } else if records
            .iter()
            .any(|record| matches!(record.budget, BudgetStanding::Budgeted(_)))
        {
            Some(Section::new(
                title,
                headed_table(
                    [
                        ColumnHeading::titled("Active (24h)"),
                        ColumnHeading::titled("Daily budget"),
                    ],
                    records.iter().map(|record| record.budgeted_record()),
                ),
            ))
        } else {
            Some(Section::new(
                title,
                headed_table(
                    [ColumnHeading::titled("Active (24h)")],
                    records.iter().map(|record| record.plain_record()),
                ),
            ))
        }
    }

    fn plain_record(&self) -> TableRecord<1> {
        let level = self.budget.severity();
        TableRecord::with_cells(
            self.total.unit.label().into_owned(),
            [ValueCell::duration(self.total.spent)],
        )
        .level(level)
    }

    fn budgeted_record(&self) -> TableRecord<2> {
        let level = self.budget.severity();
        TableRecord::with_cells(
            self.total.unit.label().into_owned(),
            [
                ValueCell::duration(self.total.spent),
                match &self.budget {
                    BudgetStanding::Budgeted(status) => ValueCell::duration(status.budget),
                    BudgetStanding::Unbudgeted => ValueCell::Blank,
                },
            ],
        )
        .level(level)
    }
}

impl DailyTotalsGauge {
    pub fn quotas(&self) -> Vec<Quota> {
        self.units
            .iter()
            .map(|record| &record.budget)
            .chain(once(&self.screen_budget))
            .filter_map(|budget| match budget {
                BudgetStanding::Budgeted(status) => Some(status.quota.clone()),
                BudgetStanding::Unbudgeted => None,
            })
            .collect()
    }
}

impl DailyTotalsGauge {
    pub fn indicator(&self, locale: Locale) -> GroupIndicator<'_> {
        let coarse = |length| duration_text(length, locale);
        let spent = coarse(self.screen_spent);
        let label = match &self.screen_budget {
            BudgetStanding::Budgeted(status) => format!("{spent} / {}", coarse(status.budget)),
            BudgetStanding::Unbudgeted => spent,
        };
        let gauge = match self.worst() {
            Some(quota) => ReadoutGauge::Gauged {
                fill: quota.used,
                live: quota.used,
            },
            None => ReadoutGauge::Ungauged,
        };
        let quotas = self.quotas();
        let state = Quota::worst_severity(&quotas).unwrap_or_default().into();
        GroupIndicator::glyph_only(
            "Daily focus budget",
            Phosphor::HourglassMedium(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge,
            badge: ReadoutBadge::Unbadged,
        })
        .with_alerts(IndicatorAlerts {
            quota_windows: quotas,
            ..IndicatorAlerts::default()
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        self.sections().into_iter().collect::<Vec<_>>()
    }
}
