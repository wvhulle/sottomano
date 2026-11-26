use std::collections::HashMap;

use sottomano_compositor_model::compositor_output_facts::OutputInfo;
use sottomano_desktop_sources::screen_brightness_color::clightd_backlight::{
    Backlight, BacklightKind,
};
use sottomano_toml_config::Config;

use super::{
    MonitorBusRequest, MonitorGroup, MonitorInteraction, MonitorSource,
    output_details::ConnectedOutput,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, slider_control::SliderControl},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

pub fn face(backlight: &Backlight) -> Phosphor {
    match backlight.kind {
        BacklightKind::Internal => Phosphor::Laptop(Weight::Regular),
        BacklightKind::External => Phosphor::Desktop(Weight::Regular),
    }
}

pub fn hover_card(
    backlight: &Backlight,
    brightness: ValueCell,
    output: Option<&ConnectedOutput>,
) -> Vec<Section> {
    let kind = match backlight.kind {
        BacklightKind::Internal => "Internal",
        BacklightKind::External => "External (DDC)",
    };
    let display_entries = [
        LabeledValue::new("Display", backlight.serial.as_str()),
        LabeledValue::new("Type", kind),
        LabeledValue::new("Brightness", brightness),
    ];
    let output_entries = output.into_iter().flat_map(ConnectedOutput::entries);
    vec![
        Section::new(
            "Display",
            dictionary_table(display_entries.into_iter().chain(output_entries)),
        ),
        Section::hardware_identity(output.iter().flat_map(|output| output.hardware_entries())),
    ]
}

impl MonitorGroup {
    pub(super) fn backlight_rows<'view>(
        &'view self,
        config: &'view Config,
        outputs: &'view HashMap<String, OutputInfo>,
    ) -> Vec<GroupIndicator<'view>> {
        self.sliders
            .iter()
            .map(|(backlight_serial, slider)| {
                let message_serial = backlight_serial.clone();
                GroupIndicator::slider(slider, config, move |control_message| {
                    MonitorInteraction::Backlight(message_serial.clone(), control_message)
                })
                .with_hover_card(Box::new(move |_| {
                    self.backlight_hover_card(backlight_serial, slider, outputs)
                }))
                .refreshed_by(MonitorInteraction::OnBuses(
                    MonitorBusRequest::Refresh(MonitorSource::Backlights),
                ))
            })
            .collect()
    }

    fn backlight_hover_card(
        &self,
        serial: &str,
        brightness_slider: &SliderControl,
        outputs: &HashMap<String, OutputInfo>,
    ) -> Vec<Section> {
        let Some(backlight) = self.backlights.get().and_then(|backlights| {
            backlights
                .iter()
                .find(|known_backlight| known_backlight.serial == serial)
        }) else {
            return Vec::new();
        };
        self.backlights.hover_card_with(hover_card(
            backlight,
            brightness_slider.readout_cell(),
            ConnectedOutput::driving(backlight, outputs).as_ref(),
        ))
    }
}
