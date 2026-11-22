use pipewire::spa::utils::dict::DictRef;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputKind {
    #[default]
    Speaker,
    Headphones,
    Headset,
    Display,
}

impl OutputKind {
    pub(super) fn described_props_kind(props: &DictRef, icon_key: &str) -> Option<Self> {
        props
            .get("device.form-factor")
            .and_then(Self::form_factor_kind)
            .or_else(|| props.get(icon_key).and_then(Self::icon_name_kind))
    }

    fn icon_name_kind(icon_name: &str) -> Option<Self> {
        if icon_name.contains("headset") {
            Some(Self::Headset)
        } else if icon_name.contains("headphone") {
            Some(Self::Headphones)
        } else if icon_name.contains("display") || icon_name.contains("video") {
            Some(Self::Display)
        } else if icon_name.contains("speaker") {
            Some(Self::Speaker)
        } else {
            None
        }
    }

    pub(super) fn port_type_kind(port_type: &str) -> Option<Self> {
        match port_type {
            "headphones" => Some(Self::Headphones),
            "headset" | "handset" | "handsfree" => Some(Self::Headset),
            "hdmi" | "displayport" => Some(Self::Display),
            _ => None,
        }
    }

    fn form_factor_kind(form_factor: &str) -> Option<Self> {
        match form_factor {
            "headphone" => Some(Self::Headphones),
            "headset" | "hands-free" | "handset" => Some(Self::Headset),
            "tv" | "monitor" => Some(Self::Display),
            "speaker" | "internal" | "computer" | "portable" | "hifi" | "car" => {
                Some(Self::Speaker)
            }
            _ => None,
        }
    }
}

impl From<&str> for OutputKind {
    fn from(node_name: &str) -> Self {
        let name = node_name.to_ascii_lowercase();
        if name.contains("headphone") {
            Self::Headphones
        } else if name.contains("headset") {
            Self::Headset
        } else if name.contains("hdmi") || name.contains("displayport") {
            Self::Display
        } else {
            Self::Speaker
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_kind_helpers_map_pipewire_vocabulary() {
        let ucm = "alsa_output.pci-0000_00_1f.3-platform-sof_sdw.HiFi__";
        assert_eq!(
            OutputKind::from(format!("{ucm}Headphones__sink").as_str()),
            OutputKind::Headphones
        );
        assert_eq!(
            OutputKind::from(format!("{ucm}HDMI1__sink").as_str()),
            OutputKind::Display
        );
        assert_eq!(
            OutputKind::from("bluez_output.00_11_22.1"),
            OutputKind::Speaker
        );
        assert_eq!(
            OutputKind::form_factor_kind("headset"),
            Some(OutputKind::Headset)
        );
        assert_eq!(OutputKind::form_factor_kind("unheard-of"), None);
        assert_eq!(
            OutputKind::icon_name_kind("audio-headset-bluetooth"),
            Some(OutputKind::Headset)
        );
        assert_eq!(
            OutputKind::icon_name_kind("audio-headphones"),
            Some(OutputKind::Headphones)
        );
        assert_eq!(OutputKind::icon_name_kind("audio-card-analog-usb"), None);
        assert_eq!(
            OutputKind::port_type_kind("headphones"),
            Some(OutputKind::Headphones)
        );
        assert_eq!(OutputKind::port_type_kind("speaker"), None);
    }
}
