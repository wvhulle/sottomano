use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::disk_storage::{
    mounted_filesystem_usage::FilesystemUsage, udev_volume_identity::VolumePropertyKind,
};

use crate::{
    clock::Clock,
    group_items::group_item_reading::{GroupItemReading, PanelVisibility},
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(super) struct VolumeIdentityRow<'a> {
    pub usage: &'a FilesystemUsage,
}

fn property_title(kind: VolumePropertyKind) -> &'static str {
    match kind {
        VolumePropertyKind::Version => "On-disk format",
        VolumePropertyKind::Label => "Label",
        VolumePropertyKind::Uuid => "UUID",
    }
}

impl GroupItemReading for VolumeIdentityRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Volume".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::IdentificationCard(Weight::Regular).into()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        PanelVisibility::Hidden
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        let volume = &self.usage.volume;
        let fs_type = &self.usage.mount.fs_type;
        let format_text = match volume.value_of(VolumePropertyKind::Version) {
            Some(version) if version.starts_with(|first: char| first.is_ascii_digit()) => {
                format!("{fs_type} {version}")
            }
            Some(named_version) => named_version.to_owned(),
            None => fs_type.clone(),
        };
        Some(match volume.value_of(VolumePropertyKind::Label) {
            Some(volume_label) => format!("{format_text}  “{volume_label}”"),
            None => format_text,
        })
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            [LabeledValue::new(
                "Filesystem",
                self.usage.mount.fs_type.clone(),
            )]
            .into_iter()
            .chain(self.usage.volume.properties.iter().map(|property| {
                LabeledValue::new(property_title(property.kind), property.value.clone())
            }))
            .chain([LabeledValue::new(
                "Block size",
                Quantity::size(self.usage.block_size),
            )]),
        )
    }
}
