use std::collections::HashMap;

use crate::{
    data::source::{
        backlight::{Backlight, Kind},
        niri::{OutputInfo, OutputMode},
    },
    widgets::{hardware_section::HardwareRows, tooltip_body::Row},
};

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
                Kind::Internal => INTERNAL_CONNECTOR_PREFIXES
                    .iter()
                    .any(|prefix| connector.starts_with(prefix)),
                Kind::External => info.serial.as_ref() == Some(&backlight.serial),
            })
            .map(|(connector, info)| Self { connector, info })
    }

    pub fn rows(&self) -> Vec<Row> {
        let mode = self.info.mode;
        [
            ("Output", Some(self.connector.to_owned())),
            (
                "Resolution",
                mode.map(|OutputMode { width, height, .. }| format!("{width}x{height}")),
            ),
            (
                "Refresh",
                mode.map(|OutputMode { refresh_hz, .. }| format!("{refresh_hz:.2} Hz")),
            ),
            ("Scale", Some(format!("{:.2}", self.info.scale))),
        ]
        .map(Row::from)
        .into()
    }
}

impl HardwareRows for ConnectedOutput<'_> {
    fn hardware_rows(&self) -> Vec<Row> {
        [
            ("Make", &self.info.make),
            ("Model", &self.info.model),
            ("Serial", &self.info.serial),
        ]
        .map(Row::from)
        .into()
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
            kind: Kind::Internal,
            percent: 50,
        }
    }

    #[test]
    fn readings_and_hardware_are_listed_apart() {
        let outputs = laptop_panel();
        let output =
            ConnectedOutput::driving(&internal_backlight(), &outputs).expect("internal panel");
        assert_eq!(
            Section::untitled(&output.rows()),
            Section::untitled(&[
                ("Output", "eDP-1"),
                ("Resolution", "1920x1200"),
                ("Refresh", "60.00 Hz"),
                ("Scale", "1.20"),
            ])
        );
        assert_eq!(
            output.hardware_section(),
            Section::titled("Hardware", &[("Make", "LG Display")])
        );
    }

    #[test]
    fn placeholder_model_is_not_shown() {
        let outputs = laptop_panel();
        let hardware = ConnectedOutput::driving(&internal_backlight(), &outputs)
            .expect("internal panel")
            .hardware_section();
        let SectionBody::Rows(rows) = hardware.body() else {
            panic!("expected rows");
        };
        assert!(rows.iter().all(|row| *row.key() != Cell::from("Model")));
    }

    #[test]
    fn external_backlight_without_matching_serial_has_no_output() {
        let outputs = laptop_panel();
        let external = Backlight {
            serial: "ABC123".to_owned(),
            kind: Kind::External,
            percent: 50,
        };
        assert!(ConnectedOutput::driving(&external, &outputs).is_none());
    }
}
