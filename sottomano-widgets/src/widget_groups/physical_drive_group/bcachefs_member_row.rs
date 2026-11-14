use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::disk_storage::bcachefs_filesystem_space::BcachefsDeviceSpace;
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill, group_item_reading::GroupItemReading,
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

const WRITABLE_MEMBER_STATE: &str = "rw";

pub(super) struct BcachefsMemberRow<'a> {
    pub device: &'a BcachefsDeviceSpace,
}

impl BcachefsMemberRow<'_> {
    fn writable_severity(&self) -> Severity {
        if self.device.state == WRITABLE_MEMBER_STATE {
            Severity::Normal
        } else {
            Severity::Warning
        }
    }

    fn used_percent(&self) -> i64 {
        let capacity = self.device.capacity.as_u64();
        if capacity == 0 {
            return 0;
        }
        let used = capacity
            .checked_sub(self.device.free.as_u64())
            .expect("the source rejects members with more reusable than total buckets");
        (used as f64 * 100.0 / capacity as f64).round() as i64
    }
}

impl GroupItemReading for BcachefsMemberRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        self.device.partition.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::HardDrives(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.writable_severity().into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(format!(
            "{} free",
            Quantity::size(self.device.free).localized_text(locale)
        ))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let metadata_text = if self.device.holds_metadata {
            "data and metadata"
        } else {
            "data only"
        };
        HoverCardBody::dictionary([
            LabeledValue::new("Device label", self.device.label.clone()),
            LabeledValue::new("Partition", self.device.partition.clone()),
            LabeledValue::new("State", self.device.state.clone()).level(self.writable_severity()),
            LabeledValue::new("Used", Quantity::share(percent_fill(self.used_percent()))),
            LabeledValue::new("Free", Quantity::size(self.device.free)),
            LabeledValue::new("Capacity", Quantity::size(self.device.capacity)),
            LabeledValue::new("Holds", metadata_text),
        ])
    }
}
