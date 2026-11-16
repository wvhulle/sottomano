use sottomano_desktop_sources::disk_storage::{
    mounted_filesystem_usage::FilesystemUsage, udev_volume_identity::VolumePropertyKind,
};

use crate::{
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
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

impl<'a> VolumeIdentityRow<'a> {
    pub(super) fn indicator(self) -> GroupIndicator<'a> {
        let volume = &self.usage.volume;
        let fs_type = &self.usage.mount.fs_type;
        let format_text = match volume.value_of(VolumePropertyKind::Version) {
            Some(version) if version.starts_with(|first: char| first.is_ascii_digit()) => {
                format!("{fs_type} {version}")
            }
            Some(named_version) => named_version.to_owned(),
            None => fs_type.clone(),
        };
        let label = match volume.value_of(VolumePropertyKind::Label) {
            Some(volume_label) => format!("{format_text}  “{volume_label}”"),
            None => format_text,
        };
        GroupIndicator::glyph_only(
            "Volume",
            Phosphor::IdentificationCard(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_panel_visibility(PanelVisibility::Hidden)
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Identity",
            dictionary_table(
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
            ),
        )]
    }
}
