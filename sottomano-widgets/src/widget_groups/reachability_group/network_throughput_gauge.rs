use chrono::Locale;
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
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutGauge, RowReadout},
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

fn throughput_state(throughput: &Throughput) -> ReadingDisplayState {
    let severity = severity(throughput);
    if throughput.interfaces.is_empty()
        || (throughput.link_usage() == LinkUsage::Idle && severity == Severity::Normal)
    {
        ReadingDisplayState::Off
    } else {
        severity.into()
    }
}

pub(super) fn throughput_hover_card(throughput: &Throughput, _clock: Clock) -> Vec<Section> {
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

pub(super) fn throughput_reading_row(
    throughput: &Throughput,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
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
        Box::new(move |card_clock| throughput_hover_card(throughput, card_clock)),
    );
    ReadingRow {
        readout: RowReadout {
            gauge: ReadoutGauge::Gauged {
                fill: Ratio::new::<ratio>(f64::from(throughput.peak_fraction())),
                live: Ratio::new::<ratio>(f64::from(throughput.current_fraction())),
            },
            ..base.readout
        },
        ..base
    }
}

pub(super) fn interface_throughput_hover_card(
    throughput: &InterfaceThroughput,
    _clock: Clock,
) -> Vec<Section> {
    vec![Section::new(
        "Rate",
        dictionary_table([
            LabeledValue::new("Down", Quantity::transfer_rate(throughput.down.rate)),
            LabeledValue::new("Up", Quantity::transfer_rate(throughput.up.rate)),
        ]),
    )]
}

pub(super) fn interface_throughput_reading_row(
    throughput: &InterfaceThroughput,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    ReadingRow {
        panel_visibility: PanelVisibility::Hidden,
        ..ReadingRow::glyph_only(
            InterfaceThroughput::NAME,
            Phosphor::ArrowsDownUp(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(move |card_clock| interface_throughput_hover_card(throughput, card_clock)),
        )
    }
}
