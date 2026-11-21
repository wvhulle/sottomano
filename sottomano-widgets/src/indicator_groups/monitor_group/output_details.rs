use std::collections::HashMap;

use sottomano_compositor_model::compositor_output_facts::{
    EdidProductText, OutputInfo, OutputMode, OutputModeReport,
};
use sottomano_desktop_sources::screen_brightness_color::clightd_backlight::{
    Backlight, BacklightKind,
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
                BacklightKind::External => {
                    matches!(&info.serial, EdidProductText::Reported(serial) if *serial == backlight.serial)
                }
            })
            .map(|(connector, info)| Self { connector, info })
    }

    pub fn entries(&self) -> Vec<LabeledValue> {
        let mut entries = vec![LabeledValue::new("Output", self.connector)];
        if let OutputModeReport::Current(OutputMode {
            width,
            height,
            refresh_hz,
        }) = self.info.mode
        {
            entries.push(LabeledValue::new("Resolution", format!("{width}x{height}")));
            entries.push(LabeledValue::new(
                "Refresh",
                Quantity::measured(refresh_hz, DisplayUnit::Hertz, 2),
            ));
        }
        entries.push(LabeledValue::new(
            "Scale",
            Quantity::measured(self.info.scale, DisplayUnit::Unitless, 2),
        ));
        entries
    }
}

impl ConnectedOutput<'_> {
    pub fn hardware_entries(&self) -> Vec<LabeledValue> {
        let mut entries = Vec::new();
        if let EdidProductText::Reported(make) = &self.info.make {
            entries.push(LabeledValue::logo_prefixed(
                "Make",
                PnpManufacturerName {
                    manufacturer_name: make,
                }
                .vendor_logo(),
                make.as_str(),
            ));
        }
        for (key, value) in [("Model", &self.info.model), ("Serial", &self.info.serial)] {
            if let EdidProductText::Reported(value) = value {
                entries.push(LabeledValue::new(key, value.as_str()));
            }
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{glyph_icons::simple_icons::SimpleIcon, widgets::value_cells::ValueCell};

    fn laptop_panel() -> HashMap<String, OutputInfo> {
        HashMap::from([(
            "eDP-1".to_owned(),
            OutputInfo {
                make: EdidProductText::Reported("LG Display".to_owned()),
                model: EdidProductText::Unreported,
                serial: EdidProductText::Unreported,
                mode: OutputModeReport::Current(OutputMode {
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
    fn indicators_and_hardware_are_listed_apart() {
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
            output.hardware_entries(),
            [LabeledValue::new(
                "Make",
                ValueCell::BrandedText {
                    text: "LG Display".to_owned(),
                    glyph: SimpleIcon::LG.into(),
                }
            )]
        );
    }

    #[test]
    fn placeholder_model_is_not_shown() {
        let outputs = laptop_panel();
        let hardware_entries = ConnectedOutput::driving(&internal_backlight(), &outputs)
            .expect("internal panel")
            .hardware_entries();
        assert!(
            hardware_entries
                .iter()
                .all(|entry| entry.key != ValueCell::from("Model"))
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
