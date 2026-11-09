use chrono::Locale;
use sottomano_desktop_sources::disk_storage::mounted_filesystem_usage::{
    FilesystemUsage, MountAccess,
};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
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

impl GroupItemReading for MountAccessRow<'_> {
    fn name(&self) -> &'static str {
        "Mount"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::HardDrive(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(access_text(self.usage.access).to_owned())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let joined = |parts: Vec<String>| parts.join("  ");
        HoverCardBody::dictionary([
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
        ])
    }
}
