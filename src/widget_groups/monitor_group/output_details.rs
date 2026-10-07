use std::collections::HashMap;

use crate::{
    data::source::{
        backlight::{Backlight, Kind},
        niri::{OutputInfo, OutputMode},
    },
    widgets::{
        hardware_section::{HardwareRows, reported_rows},
        tooltip_body::{Row, row},
    },
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
        let mut rows = vec![row("Output", self.connector.to_owned())];
        rows.extend(self.geometry_rows());
        rows
    }

    fn geometry_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        if let Some(OutputMode {
            width,
            height,
            refresh_hz,
        }) = self.info.mode
        {
            rows.push(row("Resolution", format!("{width}x{height}")));
            rows.push(row("Refresh", format!("{refresh_hz:.2} Hz")));
        }
        rows.push(row("Scale", format!("{:.2}", self.info.scale)));
        rows
    }
}

impl HardwareRows for ConnectedOutput<'_> {
    fn hardware_rows(&self) -> Vec<Row> {
        reported_rows([
            ("Make", self.info.make.as_ref()),
            ("Model", self.info.model.as_ref()),
            ("Serial", self.info.serial.as_ref()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tooltip_body::Cell;

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
            output.rows(),
            [
                row("Output", "eDP-1"),
                row("Resolution", "1920x1200"),
                row("Refresh", "60.00 Hz"),
                row("Scale", "1.20"),
            ]
        );
        assert_eq!(output.hardware_rows(), [row("Make", "LG Display")]);
    }

    #[test]
    fn placeholder_model_is_not_shown() {
        let outputs = laptop_panel();
        let hardware = ConnectedOutput::driving(&internal_backlight(), &outputs)
            .expect("internal panel")
            .hardware_rows();
        assert!(hardware.iter().all(|row| row.key != Cell::from("Model")));
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
