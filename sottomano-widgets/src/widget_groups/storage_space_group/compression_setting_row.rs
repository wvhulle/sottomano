use std::borrow::Cow;

use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::disk_storage::{
    bcachefs_filesystem_space::BcachefsSpace, block_filesystem_mounts::Compression,
};

use crate::{
    clock::Clock,
    group_items::group_item_reading::{GroupItemReading, PanelVisibility},
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::{Quantity, localized_decimal},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const BCACHEFS_NO_COMPRESSION: &str = "none";

pub(super) enum CompressionSettingRow<'a> {
    Btrfs(&'a Compression),
    Bcachefs(&'a BcachefsSpace),
}

impl CompressionSettingRow<'_> {
    pub(super) fn is_enabled(&self) -> bool {
        match self {
            CompressionSettingRow::Btrfs(compression) => **compression != Compression::Disabled,
            CompressionSettingRow::Bcachefs(space) => {
                space.compression != BCACHEFS_NO_COMPRESSION
                    || space.background_compression != BCACHEFS_NO_COMPRESSION
            }
        }
    }

    fn algorithm_text(&self) -> String {
        match self {
            CompressionSettingRow::Btrfs(compression) => compression.to_string(),
            CompressionSettingRow::Bcachefs(space)
                if space.background_compression == space.compression
                    || space.background_compression == BCACHEFS_NO_COMPRESSION =>
            {
                space.compression.clone()
            }
            CompressionSettingRow::Bcachefs(space) => format!(
                "{}, {} in background",
                space.compression, space.background_compression
            ),
        }
    }
}

fn compression_ratio(space: &BcachefsSpace) -> Option<f64> {
    let extents = space.compressed_extents;
    (extents.compressed > ByteSize::default())
        .then(|| extents.uncompressed.as_u64() as f64 / extents.compressed.as_u64() as f64)
}

impl GroupItemReading for CompressionSettingRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Compression".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowsInSimple(Weight::Regular).into()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        PanelVisibility::Hidden
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        let ratio = match self {
            CompressionSettingRow::Bcachefs(space) => compression_ratio(space),
            CompressionSettingRow::Btrfs(_) => None,
        };
        Some(match ratio {
            Some(ratio) => format!(
                "{}  {}×",
                self.algorithm_text(),
                localized_decimal(ratio, 1, locale)
            ),
            None => self.algorithm_text(),
        })
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let compressed_sizes = match self {
            CompressionSettingRow::Bcachefs(space)
                if space.compressed_extents.compressed > ByteSize::default() =>
            {
                vec![
                    LabeledValue::new(
                        "Before compression",
                        Quantity::size(space.compressed_extents.uncompressed),
                    ),
                    LabeledValue::new(
                        "After compression",
                        Quantity::size(space.compressed_extents.compressed),
                    ),
                ]
            }
            CompressionSettingRow::Bcachefs(_) | CompressionSettingRow::Btrfs(_) => Vec::new(),
        };
        HoverCardBody::dictionary(
            [LabeledValue::new("Compression", self.algorithm_text())]
                .into_iter()
                .chain(compressed_sizes),
        )
    }
}
