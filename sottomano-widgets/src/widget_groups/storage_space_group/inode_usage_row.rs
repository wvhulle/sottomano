use chrono::Locale;
use sottomano_desktop_sources::disk_storage::mounted_filesystem_usage::InodeCount;
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill,
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
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

impl GroupItemReading for InodeUsageRow {
    fn name(&self) -> &'static str {
        "Inodes"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Files(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.severity().into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Running out of inodes",
        }
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(format!(
            "{} free",
            Quantity::compact_count(self.inodes.free).localized_text(locale)
        ))
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.used_percent()))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Used", Quantity::share(percent_fill(self.used_percent()))),
            LabeledValue::new("Free", Quantity::count(self.inodes.free)).level(self.severity()),
            LabeledValue::new("Total", Quantity::count(self.inodes.total)),
        ])
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
