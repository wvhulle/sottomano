use std::fs;

use super::block_filesystem_mounts::KernelDevice;
use crate::host_machine_identity::udev_hardware_identity::UdevProperties;

const VOLUME_PROPERTY_KEYS: [(VolumePropertyKind, &str); 3] = [
    (VolumePropertyKind::Version, "ID_FS_VERSION"),
    (VolumePropertyKind::Label, "ID_FS_LABEL"),
    (VolumePropertyKind::Uuid, "ID_FS_UUID"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumePropertyKind {
    Version,
    Label,
    Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeProperty {
    pub kind: VolumePropertyKind,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VolumeIdentity {
    pub properties: Vec<VolumeProperty>,
}

impl VolumeIdentity {
    #[must_use]
    pub fn of_devices(devices: &[KernelDevice]) -> Self {
        devices
            .iter()
            .filter_map(|device| {
                let device_number_path = device.sysfs_dir().join("dev");
                fs::read_to_string(&device_number_path)
                    .inspect_err(|read_failure| {
                        tracing::debug!(
                            "volume identity: reading {} failed: {read_failure}",
                            device_number_path.display()
                        );
                    })
                    .ok()
            })
            .map(|major_minor| Self::from(&UdevProperties::block_device(major_minor.trim())))
            .find(|identity| !identity.properties.is_empty())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn value_of(&self, kind: VolumePropertyKind) -> Option<&str> {
        self.properties
            .iter()
            .find(|property| property.kind == kind)
            .map(|property| property.value.as_str())
    }
}

impl From<&UdevProperties> for VolumeIdentity {
    fn from(udev_properties: &UdevProperties) -> Self {
        VolumeIdentity {
            properties: VOLUME_PROPERTY_KEYS
                .iter()
                .filter_map(|(kind, key)| {
                    udev_properties
                        .value(key)
                        .map(|value| VolumeProperty { kind: *kind, value })
                })
                .collect(),
        }
    }
}
