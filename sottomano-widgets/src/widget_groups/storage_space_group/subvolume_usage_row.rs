use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::disk_storage::bcachefs_subvolume_usage::{
    BcachefsSubvolume, BcachefsSubvolumeUsage, MountAccess, SubvolumeMount,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill,
        group_item_reading::{GroupItemReading, PanelVisibility},
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(super) struct SubvolumeUsageRow<'a> {
    pub subvolume: &'a BcachefsSubvolume,
    pub pool_total: ByteSize,
}

impl SubvolumeUsageRow<'_> {
    fn pool_share(&self) -> Ratio {
        if self.pool_total == ByteSize::default() {
            return percent_fill(0);
        }
        let whole_percent =
            self.subvolume.used.as_u64() as f64 * 100.0 / self.pool_total.as_u64() as f64;
        percent_fill(whole_percent.round().min(100.0) as i64)
    }
}

impl GroupItemReading for SubvolumeUsageRow<'_> {
    fn name(&self) -> &'static str {
        "Subvolume"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::FolderSimple(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(format!(
            "{}  {}",
            self.subvolume.path.display(),
            Quantity::size(self.subvolume.used).localized_text(locale)
        ))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let mounted_at = self
            .subvolume
            .mounts
            .iter()
            .map(mount_description)
            .collect::<Vec<_>>()
            .join("  ");
        let snapshot_entries = (self.subvolume.snapshot_count > 0)
            .then(|| {
                [
                    LabeledValue::new("Snapshots", self.subvolume.snapshot_count.to_string()),
                    LabeledValue::new(
                        "Only in snapshots",
                        Quantity::size(self.subvolume.only_in_snapshots),
                    ),
                ]
            })
            .into_iter()
            .flatten();
        HoverCardBody::dictionary(
            [
                LabeledValue::new("Subvolume", self.subvolume.path.display().to_string()),
                LabeledValue::new("Mounted at", mounted_at),
                LabeledValue::new("Used", Quantity::size(self.subvolume.used)),
                LabeledValue::new("Share of filesystem", Quantity::share(self.pool_share())),
            ]
            .into_iter()
            .chain(snapshot_entries),
        )
    }
}

fn mount_description(mount: &SubvolumeMount) -> String {
    match mount.access {
        MountAccess::ReadWrite => mount.mount_point.display().to_string(),
        MountAccess::ReadOnly => format!("{} (read-only)", mount.mount_point.display()),
    }
}

impl GroupItemReading for BcachefsSubvolumeUsage {
    fn name(&self) -> &'static str {
        "Subvolumes"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::FolderSimple(Weight::Regular).into()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        PanelVisibility::Hidden
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(self.subvolumes.iter().map(|subvolume| {
            LabeledValue::new(
                subvolume.path.display().to_string(),
                Quantity::size(subvolume.used),
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uom::si::ratio::percent;

    use super::*;

    #[test]
    fn row_has_no_gauge_that_would_replace_its_label() {
        let subvolume = BcachefsSubvolume {
            path: PathBuf::from("/home"),
            used: ByteSize::gib(1),
            only_in_snapshots: ByteSize::b(0),
            snapshot_count: 0,
            mounts: Vec::new(),
        };
        let row = SubvolumeUsageRow {
            subvolume: &subvolume,
            pool_total: ByteSize::gib(4),
        };
        assert!(row.gauge().is_none());
    }

    fn row_share(used: ByteSize, pool_total: ByteSize) -> i64 {
        let subvolume = BcachefsSubvolume {
            path: PathBuf::from("/home"),
            used,
            only_in_snapshots: ByteSize::b(0),
            snapshot_count: 0,
            mounts: Vec::new(),
        };
        SubvolumeUsageRow {
            subvolume: &subvolume,
            pool_total,
        }
        .pool_share()
        .get::<percent>()
        .round() as i64
    }

    #[test]
    fn share_is_of_the_whole_filesystem() {
        assert_eq!(row_share(ByteSize::gib(1), ByteSize::gib(4)), 25);
    }

    #[test]
    fn share_stays_within_bounds() {
        assert_eq!(row_share(ByteSize::gib(8), ByteSize::gib(4)), 100);
        assert_eq!(row_share(ByteSize::gib(1), ByteSize::b(0)), 0);
    }

    #[test]
    fn read_only_mounts_are_marked() {
        let read_only = SubvolumeMount {
            mount_point: PathBuf::from("/nix/store"),
            access: MountAccess::ReadOnly,
        };
        assert_eq!(mount_description(&read_only), "/nix/store (read-only)");
    }
}
