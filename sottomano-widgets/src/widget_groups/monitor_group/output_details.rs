use std::collections::HashMap;

use sottomano_sources::{
    desktop_windows_apps::niri_window_snapshot::{OutputInfo, OutputMode},
    screen_brightness_color::clightd_backlight::{Backlight, BacklightKind},
};

use crate::widgets::tooltip_body::KeyValue;

const INTERNAL_CONNECTOR_PREFIXES: [&str; 3] = ["eDP", "LVDS", "DSI"];

pub struct ConnectedOutput<'a> {
    pub connector: &'a str,
    pub info: &'a OutputInfo,
}

impl<'a> ConnectedOutput<'a> {
    pub fn driving(
        backlight: &Backlight,
        outputs: &'a HashMap<String, OutputInfo>,
    ) -> Option<Self> {
        outputs
            .iter()
            .find(|(connector, info)| match backlight.kind {
                BacklightKind::Internal => INTERNAL_CONNECTOR_PREFIXES
                    .iter()
                    .any(|prefix| connector.starts_with(prefix)),
                BacklightKind::External => info.serial.as_ref() == Some(&backlight.serial),
            })
            .map(|(connector, info)| Self { connector, info })
    }

    pub fn entries(&self) -> Vec<KeyValue> {
        let mode_entries = self.info.mode.into_iter().flat_map(
            |OutputMode {
                 width,
                 height,
                 refresh_hz,
             }| {
                [
                    KeyValue::new("Resolution", format!("{width}x{height}")),
                    KeyValue::new("Refresh", format!("{refresh_hz:.2} Hz")),
                ]
            },
        );
        [KeyValue::new("Output", self.connector)]
            .into_iter()
            .chain(mode_entries)
            .chain([KeyValue::new("Scale", format!("{:.2}", self.info.scale))])
            .collect()
    }
}

impl ConnectedOutput<'_> {
    pub fn hardware_entries(&self) -> Vec<KeyValue> {
        [
            ("Make", &self.info.make),
            ("Model", &self.info.model),
            ("Serial", &self.info.serial),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tooltip_body::{Cell, Section, SectionBody};

    fn laptop_panel() -> HashMap<String, OutputInfo> {
        HashMap::from([(
            "eDP-1".to_owned(),
            OutputInfo {
                make: Some("LG Display".to_owned()),
                model: None,
                serial: None,
                mode: Some(OutputMode {
                    width: 1920,
                    height: 1200,
                    refresh_hz: 60.0,
                }),
                scale: 1.2,
                logical_height: 1000,
            },
        )])
    }

    fn internal_backlight() -> Backlight {
        Backlight {
            serial: "intel_backlight".to_owned(),
            kind: BacklightKind::Internal,
            percent: 50,
        }
    }

    #[test]
    fn readings_and_hardware_are_listed_apart() {
        let outputs = laptop_panel();
        let output =
            ConnectedOutput::driving(&internal_backlight(), &outputs).expect("internal panel");
        assert_eq!(
            output.entries(),
            [
                KeyValue::new("Output", "eDP-1"),
                KeyValue::new("Resolution", "1920x1200"),
                KeyValue::new("Refresh", "60.00 Hz"),
                KeyValue::new("Scale", "1.20"),
            ]
        );
        assert_eq!(
            Section::hardware_identity(output.hardware_entries()),
            Section::dictionary([KeyValue::new("Make", "LG Display")]).titled("Hardware")
        );
    }

    #[test]
    fn placeholder_model_is_not_shown() {
        let outputs = laptop_panel();
        let hardware = Section::hardware_identity(
            ConnectedOutput::driving(&internal_backlight(), &outputs)
                .expect("internal panel")
                .hardware_entries(),
        );
        let SectionBody::Dictionary(entries) = hardware.body() else {
            panic!("expected dictionary");
        };
        assert!(
            entries
                .iter()
                .all(|entry| *entry.key() != Cell::from("Model"))
        );
    }

    #[test]
    fn external_backlight_without_matching_serial_has_no_output() {
        let outputs = laptop_panel();
        let external = Backlight {
            serial: "ABC123".to_owned(),
            kind: BacklightKind::External,
            percent: 50,
        };
        assert!(ConnectedOutput::driving(&external, &outputs).is_none());
    }
}
