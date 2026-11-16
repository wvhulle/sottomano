use chrono::Locale;
use sottomano_desktop_sources::disk_storage::mounted_filesystem_usage::InodeCount;
use sottomano_toml_config::severity::Severity;

use crate::{
    group_indicators::{
        gauge::percent_fill,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::{IndicatorAlerts, StateToast, ToastBody, ToastProgress},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const FREE_INODES_WARN_SHARE: f64 = 0.1;
const FREE_INODES_CRITICAL_SHARE: f64 = 0.02;

pub(super) struct InodeUsageRow {
    pub inodes: InodeCount,
}

impl InodeUsageRow {
    fn free_share(&self) -> f64 {
        if self.inodes.total == 0 {
            return 1.0;
        }
        self.inodes.free as f64 / self.inodes.total as f64
    }

    fn used_percent(&self) -> i64 {
        ((1.0 - self.free_share()) * 100.0).round() as i64
    }

    fn severity(&self) -> Severity {
        let free_share = self.free_share();
        if free_share < FREE_INODES_CRITICAL_SHARE {
            Severity::Critical
        } else if free_share < FREE_INODES_WARN_SHARE {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

impl InodeUsageRow {
    pub(super) fn reading_row(self, locale: Locale) -> ReadingRow<'static> {
        let label = format!(
            "{} free",
            Quantity::compact_count(self.inodes.free).localized_text(locale)
        );
        let fill = percent_fill(self.used_percent());
        ReadingRow {
            readout: IndicatorReadout {
                label: ReadoutLabel::Labeled(label.clone()),
                gauge: ReadoutGauge::Gauged { fill, live: fill },
                badge: ReadoutBadge::Unbadged,
            },
            alerts: IndicatorAlerts {
                state_toast: StateToast::NotifyFromSeverity {
                    minimum_severity: Severity::Critical,
                    summary_text: "Running out of inodes",
                    body: ToastBody::Text(label),
                    progress: ToastProgress::Measured(fill),
                },
                ..IndicatorAlerts::default()
            },
            ..ReadingRow::glyph_only(
                "Inodes",
                Phosphor::Files(Weight::Regular).into(),
                self.severity().into(),
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Usage",
            dictionary_table([
                LabeledValue::new("Used", Quantity::share(percent_fill(self.used_percent()))),
                LabeledValue::new("Free", Quantity::count(self.inodes.free)).level(self.severity()),
                LabeledValue::new("Total", Quantity::count(self.inodes.total)),
            ]),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn severity_with(total: u64, free: u64) -> Severity {
        InodeUsageRow {
            inodes: InodeCount { total, free },
        }
        .severity()
    }

    #[test]
    fn plenty_of_free_inodes_is_normal() {
        assert_eq!(severity_with(1_000_000, 900_000), Severity::Normal);
    }

    #[test]
    fn few_free_inodes_warns() {
        assert_eq!(severity_with(1_000_000, 50_000), Severity::Warning);
    }

    #[test]
    fn nearly_no_free_inodes_is_critical() {
        assert_eq!(severity_with(1_000_000, 10_000), Severity::Critical);
    }
}
