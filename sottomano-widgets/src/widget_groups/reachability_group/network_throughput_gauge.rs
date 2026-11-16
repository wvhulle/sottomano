use sottomano_desktop_sources::{
    SnapshotSource,
    network_connectivity::interface_throughput::{
        InterfaceThroughput, LinkUsage, NetworkTrafficDirection, SCALE_END_MB_PER_S, Throughput,
    },
};
use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
};

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutGauge},
        indicator_state::IndicatorState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const HI_RES_AUDIO_STREAM_KB_PER_S: f64 = 1_152.0;
const HI_RES_VIDEO_STREAM_KB_PER_S: f64 = 3_125.0;

fn severity(throughput: &Throughput) -> Severity {
    if throughput.link_usage() == LinkUsage::Idle {
        Severity::Normal
    } else if throughput.peak_rate
        < InformationRate::new::<kilobyte_per_second>(HI_RES_AUDIO_STREAM_KB_PER_S)
    {
        Severity::Critical
    } else if throughput.peak_rate
        < InformationRate::new::<kilobyte_per_second>(HI_RES_VIDEO_STREAM_KB_PER_S)
    {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

fn throughput_state(throughput: &Throughput) -> IndicatorState {
    let severity = severity(throughput);
    if throughput.interfaces.is_empty()
        || (throughput.link_usage() == LinkUsage::Idle && severity == Severity::Normal)
    {
        IndicatorState::Off
    } else {
        severity.into()
    }
}

pub(super) fn throughput_hover_card(throughput: &Throughput) -> Vec<Section> {
    if throughput.interfaces.is_empty() {
        return vec![Section::new(
            "Rate",
            dictionary_table([LabeledValue::new("Interfaces", "none")]),
        )];
    }
    let rate_label = match throughput.direction {
        NetworkTrafficDirection::Upload => "Up",
        NetworkTrafficDirection::Download => "Down",
    };
    vec![Section::new(
        "Rate",
        dictionary_table(
            [
                LabeledValue::new(rate_label, Quantity::transfer_rate(throughput.rate)),
                LabeledValue::new("Peak", Quantity::transfer_rate(throughput.peak_rate)),
                LabeledValue::new("Interfaces", throughput.interfaces.join(", ")),
            ]
            .into_iter()
            .chain([LabeledValue::new(
                "Capacity",
                Quantity::transfer_rate(InformationRate::new::<megabyte_per_second>(
                    SCALE_END_MB_PER_S,
                )),
            )]),
        ),
    )]
}

pub(super) fn throughput_indicator(throughput: &Throughput) -> ReadingRow<'_> {
    let (name, glyph) = match throughput.direction {
        NetworkTrafficDirection::Upload => ("Upload", Phosphor::UploadSimple(Weight::Regular)),
        NetworkTrafficDirection::Download => {
            ("Download", Phosphor::DownloadSimple(Weight::Regular))
        }
    };
    let base = ReadingRow::glyph_only(
        name,
        glyph.with_weight(Weight::Regular).into(),
        throughput_state(throughput),
        Box::new(move |_| throughput_hover_card(throughput)),
    );
    ReadingRow {
        readout: IndicatorReadout {
            gauge: ReadoutGauge::Gauged {
                fill: Ratio::new::<ratio>(f64::from(throughput.peak_fraction())),
                live: Ratio::new::<ratio>(f64::from(throughput.current_fraction())),
            },
            ..base.readout
        },
        ..base
    }
}

pub(super) fn interface_throughput_hover_card(throughput: &InterfaceThroughput) -> Vec<Section> {
    vec![Section::new(
        "Rate",
        dictionary_table([
            LabeledValue::new("Down", Quantity::transfer_rate(throughput.down.rate)),
            LabeledValue::new("Up", Quantity::transfer_rate(throughput.up.rate)),
        ]),
    )]
}

pub(super) fn interface_throughput_indicator(throughput: &InterfaceThroughput) -> ReadingRow<'_> {
    ReadingRow {
        panel_visibility: PanelVisibility::Hidden,
        ..ReadingRow::glyph_only(
            InterfaceThroughput::NAME,
            Phosphor::ArrowsDownUp(Weight::Regular).into(),
            IndicatorState::Idle,
            Box::new(move |_| interface_throughput_hover_card(throughput)),
        )
    }
}
