use std::borrow::Cow;

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
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
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

impl GroupItemReading for Throughput {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(match self.direction {
            NetworkTrafficDirection::Upload => "Upload",
            NetworkTrafficDirection::Download => "Download",
        })
    }

    fn icon(&self) -> IconGlyph {
        let direction_icon = match self.direction {
            NetworkTrafficDirection::Upload => Phosphor::UploadSimple(Weight::Regular),
            NetworkTrafficDirection::Download => Phosphor::DownloadSimple(Weight::Regular),
        };
        direction_icon.with_weight(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        let severity = severity(self);
        if self.interfaces.is_empty()
            || (self.link_usage() == LinkUsage::Idle && severity == Severity::Normal)
        {
            ReadingDisplayState::Off
        } else {
            severity.into()
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<ratio>(f64::from(self.peak_fraction())))
    }

    fn live_gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<ratio>(f64::from(self.current_fraction())))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        if self.interfaces.is_empty() {
            return HoverCardBody::dictionary([LabeledValue::new("Interfaces", "none")]);
        }
        let rate_label = match self.direction {
            NetworkTrafficDirection::Upload => "Up",
            NetworkTrafficDirection::Download => "Down",
        };
        HoverCardBody::dictionary(
            [
                LabeledValue::new(rate_label, Quantity::transfer_rate(self.rate)),
                LabeledValue::new("Peak", Quantity::transfer_rate(self.peak_rate)),
                LabeledValue::new("Interfaces", self.interfaces.join(", ")),
            ]
            .into_iter()
            .chain([LabeledValue::new(
                "Capacity",
                Quantity::transfer_rate(InformationRate::new::<megabyte_per_second>(
                    SCALE_END_MB_PER_S,
                )),
            )]),
        )
    }
}

impl GroupItemReading for InterfaceThroughput {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowsDownUp(Weight::Regular).into()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        PanelVisibility::Hidden
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Down", Quantity::transfer_rate(self.down.rate)),
            LabeledValue::new("Up", Quantity::transfer_rate(self.up.rate)),
        ])
    }
}
