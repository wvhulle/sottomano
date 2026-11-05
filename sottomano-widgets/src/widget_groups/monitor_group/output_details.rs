use std::collections::HashMap;

use sottomano_desktop_sources::{
    desktop_windows_apps::niri_output_facts::{OutputInfo, OutputMode},
    screen_brightness_color::clightd_backlight::{Backlight, BacklightKind},
};

use crate::widgets::{
    localized_quantities::{DisplayUnit, Quantity},
    value_cells::LabeledValue,
    vendor_logo::{PnpManufacturerName, VendorLogo},
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
                BacklightKind::Internal => INTERNAL_CONNECTOR_PREFIXES
                    .iter()
                    .any(|prefix| connector.starts_with(prefix)),
                BacklightKind::External => info.serial.as_ref() == Some(&backlight.serial),
            })
            .map(|(connector, info)| Self { connector, info })
    }

    pub fn entries(&self) -> Vec<LabeledValue> {
        let mode_entries = self.info.mode.into_iter().flat_map(
            |OutputMode {
                 width,
                 height,
                 refresh_hz,
             }| {
                [
                    LabeledValue::new("Resolution", format!("{width}x{height}")),
                    LabeledValue::new(
                        "Refresh",
                        Quantity::measured(refresh_hz, DisplayUnit::Hertz, 2),
                    ),
                ]
            },
        );
        [LabeledValue::new("Output", self.connector)]
            .into_iter()
            .chain(mode_entries)
            .chain([LabeledValue::new(
                "Scale",
                Quantity::measured(self.info.scale, DisplayUnit::Unitless, 2),
            )])
            .collect()
    }
}

impl ConnectedOutput<'_> {
    pub fn hardware_entries(&self) -> Vec<LabeledValue> {
        let make = self.info.make.as_deref().map(|make| {
            LabeledValue::logo_prefixed("Make", PnpManufacturerName(make).vendor_logo(), make)
        });
        make.into_iter()
            .chain(
                [("Model", &self.info.model), ("Serial", &self.info.serial)]
                    .into_iter()
                    .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?))),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hover_cards::hover_card_body::{Section, SectionBody},
        widgets::{simple_icons::SimpleIcon, value_cells::ValueCell},
    };

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
                logical_width: 1600,
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
                LabeledValue::new("Output", "eDP-1"),
                LabeledValue::new("Resolution", "1920x1200"),
                LabeledValue::new("Refresh", Quantity::measured(60.0, DisplayUnit::Hertz, 2)),
                LabeledValue::new("Scale", Quantity::measured(1.2, DisplayUnit::Unitless, 2)),
            ]
        );
        assert_eq!(
            Section::hardware_identity(output.hardware_entries()),
            Section::dictionary([LabeledValue::new(
                "Make",
                ValueCell::BrandedText {
                    text: "LG Display".to_owned(),
                    glyph: SimpleIcon::LG.into(),
                }
            )])
            .titled("Hardware")
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
        let SectionBody::ColumnTable(table) = hardware.body() else {
            panic!("expected a table");
        };
        assert!(
            table
                .records()
                .all(|record| *record.key() != ValueCell::from("Model"))
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
