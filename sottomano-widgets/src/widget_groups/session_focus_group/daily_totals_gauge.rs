use std::{cmp::Ordering, iter::once};

use chrono::{DateTime, Locale, TimeDelta, Utc};
use sottomano_desktop_sources::{
    ai_usage_spend::usage_quota::Quota,
    login_session_presence::focus_time_ledger::{
        Accounting, BudgetStatus, FocusUnit, LAST_DAY, UnitTotal,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    group_items::{
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_durations::duration_text,
        ongoing_toast::ReadingAlerts,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct DailyTotalsGauge {
    computed_at: DateTime<Utc>,
    screen_spent: TimeDelta,
    screen_budget: Option<BudgetStatus>,
    units: Vec<UnitSpendRecord>,
}

impl DailyTotalsGauge {
    pub fn new(accounting: &Accounting<'_>, now: DateTime<Utc>) -> Self {
        let last_day = LAST_DAY.ending_at(now);
        Self {
            computed_at: now,
            screen_spent: accounting.screen_spent(last_day, now),
            screen_budget: accounting.screen_budget_status(now),
            units: accounting
                .unit_totals(last_day, now)
                .into_iter()
                .map(|total| UnitSpendRecord {
                    budget_status: total.budget_status(now),
                    total,
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn computed_at(&self) -> DateTime<Utc> {
        self.computed_at
    }

    fn worst(&self) -> Option<Quota> {
        self.quotas()
            .into_iter()
            .max_by(|a, b| a.used.partial_cmp(&b.used).unwrap_or(Ordering::Equal))
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
        let status = self.screen_budget.as_ref();
        let active = LabeledValue::new("Active (24h)", ValueCell::duration(self.screen_spent))
            .level(
                status
                    .map(|status| status.quota.severity())
                    .unwrap_or_default(),
            );
        let budget_entry = status
            .map(|status| LabeledValue::new("Daily budget", ValueCell::duration(status.budget)));
        Section::new(
            SectionTitle::captioned(
                "Screen".to_owned(),
                "Only active time in the last 24 hours counts, attributed to the focused window's \
                 app and project. A budget turns critical once that time exceeds it."
                    .to_owned(),
            ),
            dictionary_table(once(active).chain(budget_entry)),
        )
    }
}

struct UnitSpendRecord {
    total: UnitTotal,
    budget_status: Option<BudgetStatus>,
}

impl UnitSpendRecord {
    fn section(records: &[&Self], title: &str) -> Option<Section> {
        if records.is_empty() {
            None
        } else if records.iter().any(|record| record.budget_status.is_some()) {
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

    fn severity(&self) -> Severity {
        self.budget_status
            .as_ref()
            .map(|status| status.quota.severity())
            .unwrap_or_default()
    }

    fn plain_record(&self) -> TableRecord<1> {
        let level = self.severity();
        TableRecord::with_cells(
            self.total.unit.label().into_owned(),
            [ValueCell::duration(self.total.spent)],
        )
        .level(level)
    }

    fn budgeted_record(&self) -> TableRecord<2> {
        let level = self.severity();
        TableRecord::with_cells(
            self.total.unit.label().into_owned(),
            [
                ValueCell::duration(self.total.spent),
                self.budget_status
                    .as_ref()
                    .map_or(ValueCell::Blank, |status| {
                        ValueCell::duration(status.budget)
                    }),
            ],
        )
        .level(level)
    }
}

impl DailyTotalsGauge {
    pub fn quotas(&self) -> Vec<Quota> {
        self.units
            .iter()
            .filter_map(|record| record.budget_status.as_ref())
            .chain(&self.screen_budget)
            .map(|status| status.quota.clone())
            .collect()
    }
}

impl DailyTotalsGauge {
    pub fn reading_row(&self, locale: Locale) -> ReadingRow<'_> {
        let coarse = |length| duration_text(length, locale);
        let spent = coarse(self.screen_spent);
        let label = match &self.screen_budget {
            Some(status) => format!("{spent} / {}", coarse(status.budget)),
            None => spent,
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
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(label),
                gauge,
                badge: ReadoutBadge::Unbadged,
            },
            alerts: ReadingAlerts {
                quota_windows: quotas,
                ..ReadingAlerts::default()
            },
            ..ReadingRow::glyph_only(
                "Daily focus budget",
                Phosphor::HourglassMedium(Weight::Regular).into(),
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub fn hover_card(&self) -> Vec<Section> {
        self.sections().into_iter().collect::<Vec<_>>()
    }
}
