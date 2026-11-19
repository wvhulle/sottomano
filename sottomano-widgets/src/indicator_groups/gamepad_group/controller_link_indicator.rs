use std::time::Duration;

use sottomano_desktop_sources::{
    gamepad_controller_input::{
        accelerometer_report_timing::DeviceClock, evdev_gamepad_devices::GamepadTransport,
    },
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};
use uom::si::{f64::Frequency, frequency::megahertz, ratio::percent};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    widgets::{
        input_report_sections::{KernelErrorListing, MeasuredReportTiming, ReportHealth},
        localized_quantities::Quantity,
        value_cells::LabeledValue,
    },
};

const LOWEST_FIVE_GIGAHERTZ_MEGAHERTZ: f64 = 4900.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WiredFallback {
    Connected,
    #[default]
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BandSharing<'a> {
    NoConflict,
    SharedWithWifi {
        network_name: &'a str,
        frequency: Frequency,
        wired_fallback: WiredFallback,
    },
}

#[must_use]
pub fn wifi_band(frequency: Frequency) -> WifiBand {
    if frequency.get::<megahertz>() < LOWEST_FIVE_GIGAHERTZ_MEGAHERTZ {
        WifiBand::SharedWithBluetooth
    } else {
        WifiBand::ClearOfBluetooth
    }
}

fn two_point_four_gigahertz_channel(frequency: Frequency) -> u32 {
    ((frequency.get::<megahertz>() - 2407.0) / 5.0)
        .round()
        .max(0.0) as u32
}

fn gap_label(longest_gap: Duration) -> String {
    format!("{} ms", longest_gap.as_millis().max(1))
}

pub struct ControllerLinkIndicator<'a> {
    pub transport: GamepadTransport,
    pub report_timing: MeasuredReportTiming,
    pub kernel_errors: Vec<&'a DeviceKernelError>,
    pub band_sharing: BandSharing<'a>,
}

impl ControllerLinkIndicator<'_> {
    fn link_stability(&self) -> LinkStability {
        if self.report_timing.report_health() == ReportHealth::Degraded
            || !self.kernel_errors.is_empty()
        {
            LinkStability::Unstable
        } else {
            LinkStability::Stable
        }
    }

    fn interference_sections(&self) -> Vec<Section> {
        match self.band_sharing {
            BandSharing::SharedWithWifi {
                network_name,
                frequency,
                wired_fallback,
            } if matches!(self.transport, GamepadTransport::Bluetooth { .. }) => {
                let ethernet = match wired_fallback {
                    WiredFallback::Connected => Some(LabeledValue::new("Ethernet", "connected")),
                    WiredFallback::Absent => None,
                };
                let remedy = match wired_fallback {
                    WiredFallback::Connected => {
                        "Wi-Fi and Bluetooth share the 2.4 GHz band. Ethernet is connected, so \
                         turning Wi-Fi off frees the band for the controller."
                    }
                    WiredFallback::Absent => {
                        "Wi-Fi and Bluetooth share the 2.4 GHz band. Moving Wi-Fi to a 5 GHz \
                         network frees it for the controller."
                    }
                };
                vec![
                    Section::new(
                        "Interference",
                        dictionary_table(
                            [
                                LabeledValue::new("Wi-Fi network", network_name),
                                LabeledValue::new(
                                    "Frequency",
                                    Quantity::clock_megahertz(frequency),
                                ),
                                LabeledValue::new(
                                    "Channel",
                                    two_point_four_gigahertz_channel(frequency).to_string(),
                                ),
                            ]
                            .into_iter()
                            .chain(ethernet),
                        ),
                    ),
                    Section::new("Shared radio band", SectionWidget::paragraph(remedy)),
                ]
            }
            _ if matches!(self.transport, GamepadTransport::Bluetooth { .. })
                && self.link_stability() == LinkStability::Unstable =>
            {
                vec![Section::new(
                    "Unstable Bluetooth link",
                    SectionWidget::paragraph(
                        "Move the controller closer to the Bluetooth adapter, or connect it by \
                         cable.",
                    ),
                )]
            }
            _ => Vec::new(),
        }
    }
}

impl<'a> ControllerLinkIndicator<'a> {
    pub fn indicator(self) -> GroupIndicator<'a> {
        let (glyph, state) = if self.link_stability() == LinkStability::Unstable {
            (
                Phosphor::LinkBreak(Weight::Regular),
                IndicatorState::Warning,
            )
        } else {
            (Phosphor::Link(Weight::Regular), IndicatorState::Idle)
        };
        let label = match self.report_timing {
            MeasuredReportTiming::Pending => ReadoutLabel::Unlabeled,
            MeasuredReportTiming::Measured(timing) => {
                ReadoutLabel::Labeled(match timing.device_clock {
                    DeviceClock::Present(sent) => {
                        format!("{:.0}% lost", sent.lost_share.get::<percent>())
                    }
                    DeviceClock::Absent if timing.stalls > 0 => {
                        format!("{} stalls", timing.stalls)
                    }
                    DeviceClock::Absent => gap_label(timing.longest_gap),
                })
            }
        };
        GroupIndicator::glyph_only("Controller lag", glyph.into(), state)
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        [
            Some(
                self.report_timing
                    .report_section("Controller reports, last 2 s"),
            ),
            KernelErrorListing {
                kernel_errors: &self.kernel_errors,
            }
            .section(),
        ]
        .into_iter()
        .flatten()
        .chain(self.interference_sections())
        .collect::<Vec<_>>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBand {
    SharedWithBluetooth,
    ClearOfBluetooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkStability {
    Unstable,
    Stable,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_megahertz(value: f64) -> Frequency {
        Frequency::new::<megahertz>(value)
    }

    #[test]
    fn channels_follow_the_two_point_four_gigahertz_plan() {
        assert_eq!(two_point_four_gigahertz_channel(at_megahertz(2412.0)), 1);
        assert_eq!(two_point_four_gigahertz_channel(at_megahertz(2437.0)), 6);
        assert_eq!(two_point_four_gigahertz_channel(at_megahertz(2462.0)), 11);
    }

    #[test]
    fn only_two_point_four_gigahertz_wifi_shares_the_bluetooth_band() {
        assert_eq!(
            wifi_band(at_megahertz(2437.0)),
            WifiBand::SharedWithBluetooth
        );
        assert_eq!(wifi_band(at_megahertz(5180.0)), WifiBand::ClearOfBluetooth);
    }
}
