use sottomano_desktop_sources::disk_storage::mounted_filesystem_usage::{
    FilesystemUsage, MountAccess,
};

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(super) struct MountAccessRow<'a> {
    pub usage: &'a FilesystemUsage,
}

fn access_text(access: MountAccess) -> &'static str {
    match access {
        MountAccess::ReadWrite => "read-write",
        MountAccess::ReadOnly => "read-only",
    }
}

impl<'a> MountAccessRow<'a> {
    pub(super) fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(access_text(self.usage.access).to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Mount",
                Phosphor::HardDrive(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let joined = |parts: Vec<String>| parts.join("  ");
        vec![Section::new(
            "Mounting",
            dictionary_table([
                LabeledValue::new(
                    "Mounted at",
                    joined(
                        self.usage
                            .mount
                            .mount_points
                            .iter()
                            .map(|mount_point| mount_point.display().to_string())
                            .collect(),
                    ),
                ),
                LabeledValue::new("Access", access_text(self.usage.access)),
                LabeledValue::new("Filesystem", self.usage.mount.fs_type.clone()),
                LabeledValue::new(
                    "Devices",
                    joined(
                        self.usage
                            .mount
                            .devices
                            .iter()
                            .map(|device| device.name.clone())
                            .collect(),
                    ),
                ),
            ]),
        )]
    }
}
