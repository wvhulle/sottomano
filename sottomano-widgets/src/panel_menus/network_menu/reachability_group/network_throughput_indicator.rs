use sottomano_desktop_sources::{
    SnapshotSource,
    network_connectivity::{
        interface_throughput::{
            InterfaceThroughput, LinkUsage, NetworkTrafficDirection, SCALE_END_MB_PER_S, Throughput,
        },
        network_manager_proxies::Metering,
    },
};
use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
};

use super::{
    ReachabilityBusRequest, ReachabilityGroup, ReachabilityInteraction,
    sqlite_metered_volume::{MeteredVolumeTotals, TransferTotals},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::ReadoutGauge,
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

const INTERFACE_THROUGHPUT_GLYPH: Phosphor = Phosphor::ArrowsDownUp(Weight::Regular);

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

pub(super) fn throughput_indicator(throughput: &Throughput) -> GroupIndicator<'_> {
    let (name, glyph) = match throughput.direction {
        NetworkTrafficDirection::Upload => ("Upload", Phosphor::UploadSimple(Weight::Regular)),
        NetworkTrafficDirection::Download => {
            ("Download", Phosphor::DownloadSimple(Weight::Regular))
        }
    };
    let base = GroupIndicator::glyph_only(
        name,
        glyph.with_weight(Weight::Regular).into(),
        throughput_state(throughput),
    )
    .with_hover_card(Box::new(move |_| throughput_hover_card(throughput)));
    base.with_gauge(ReadoutGauge::Gauged {
        fill: Ratio::new::<ratio>(f64::from(throughput.peak_fraction())),
        live: Ratio::new::<ratio>(f64::from(throughput.current_fraction())),
    })
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

pub(super) fn interface_throughput_indicator(
    throughput: &InterfaceThroughput,
) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        InterfaceThroughput::NAME,
        Phosphor::ArrowsDownUp(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_panel_visibility(PanelVisibility::Hidden)
    .with_hover_card(Box::new(move |_| {
        interface_throughput_hover_card(throughput)
    }))
}

impl ReachabilityGroup {
    pub(super) fn network_throughput_listing(&self) -> IndicatorListing<'_> {
        self.throughput.indicator_listing(
            |samples| {
                self.wired_interface_throughput_indicator(interface_throughput_indicator(samples))
                    .when_shown()
                    .into_iter()
                    .chain([
                        self.direction_throughput_indicator(&samples.down),
                        self.direction_throughput_indicator(&samples.up),
                    ])
                    .collect()
            },
            INTERFACE_THROUGHPUT_GLYPH.into(),
        )
    }

    fn wired_interface_throughput_indicator<'view>(
        &'view self,
        interface_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        interface_indicator
            .with_hover_card(Box::new(move |_| {
                self.throughput
                    .sections_described_by(interface_throughput_hover_card)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }

    fn direction_throughput_indicator<'view>(
        &'view self,
        direction_throughput: &'view Throughput,
    ) -> GroupIndicator<'view> {
        throughput_indicator(direction_throughput)
            .with_hover_card(Box::new(move |_| {
                self.direction_hover_card(direction_throughput)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }

    fn direction_hover_card(&self, throughput: &Throughput) -> Vec<Section> {
        let metered_section = self
            .throughput
            .get()
            .filter(|samples| samples.metering == Metering::Metered)
            .and_then(|_| self.metered_volume.displayed_totals())
            .map(|totals| metered_section(&totals, throughput.direction));
        self.throughput.hover_card_with(
            throughput_hover_card(throughput)
                .into_iter()
                .chain(metered_section)
                .collect(),
        )
    }
}

fn metered_section(totals: &MeteredVolumeTotals, direction: NetworkTrafficDirection) -> Section {
    let in_direction = |transferred: TransferTotals| match direction {
        NetworkTrafficDirection::Download => transferred.received,
        NetworkTrafficDirection::Upload => transferred.sent,
    };
    Section::new(
        "Metered connection",
        dictionary_table([
            LabeledValue::new("Today", Quantity::size(in_direction(totals.today))),
            LabeledValue::new("This cycle", Quantity::size(in_direction(totals.cycle))),
            LabeledValue::new(
                "Counted since",
                totals.counting_since.format("%-d %b %Y").to_string(),
            ),
        ]),
    )
}
