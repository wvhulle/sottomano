use std::time::Duration;

use chrono::{DateTime, Locale, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::{
    gamepad_controller_input::{
        accelerometer_report_timing::{DeviceClock, ReportTiming},
        evdev_gamepad_devices::GamepadTransport,
    },
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};
use uom::si::{
    f64::{Frequency, Ratio},
    frequency::megahertz,
    ratio::percent,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const NOTICEABLE_LOSS_PERCENT: f64 = 5.0;
const LISTED_KERNEL_ERRORS: usize = 5;
const LOWEST_FIVE_GIGAHERTZ_MEGAHERTZ: f64 = 4900.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeasuredReportTiming {
    Pending,
    Measured(ReportTiming),
}

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
pub fn shares_bluetooth_band(frequency: Frequency) -> bool {
    frequency.get::<megahertz>() < LOWEST_FIVE_GIGAHERTZ_MEGAHERTZ
}

fn two_point_four_gigahertz_channel(frequency: Frequency) -> u32 {
    ((frequency.get::<megahertz>() - 2407.0) / 5.0)
        .round()
        .max(0.0) as u32
}

fn noticeable_loss(lost_share: Ratio) -> bool {
    lost_share.get::<percent>() >= NOTICEABLE_LOSS_PERCENT
}

fn gap_label(longest_gap: Duration) -> String {
    format!("{} ms", longest_gap.as_millis().max(1))
}

const fn warning_when(warning: bool) -> Severity {
    if warning {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

pub struct ControllerLinkReading<'a> {
    pub transport: GamepadTransport,
    pub report_timing: MeasuredReportTiming,
    pub kernel_errors: Vec<&'a DeviceKernelError>,
    pub band_sharing: BandSharing<'a>,
}

impl ControllerLinkReading<'_> {
    const fn over_bluetooth(&self) -> bool {
        matches!(self.transport, GamepadTransport::Bluetooth { .. })
    }

    fn unstable(&self) -> bool {
        let lossy_or_stalling = match self.report_timing {
            MeasuredReportTiming::Pending => false,
            MeasuredReportTiming::Measured(timing) => {
                timing.stalls > 0
                    || matches!(
                        timing.device_clock,
                        DeviceClock::Present(sent) if noticeable_loss(sent.lost_share)
                    )
            }
        };
        lossy_or_stalling || !self.kernel_errors.is_empty()
    }

    fn report_section(&self) -> Section {
        let entries = match self.report_timing {
            MeasuredReportTiming::Pending => vec![KeyValue::new("Reports", "measuring")],
            MeasuredReportTiming::Measured(timing) => {
                let arriving =
                    KeyValue::new("Arriving", Quantity::refresh_hertz(timing.arrival_rate));
                let clocked = match timing.device_clock {
                    DeviceClock::Present(sent) => vec![
                        KeyValue::new("Sent", Quantity::refresh_hertz(sent.sent_rate)),
                        arriving,
                        KeyValue::new("Lost", Quantity::share(sent.lost_share))
                            .level(warning_when(noticeable_loss(sent.lost_share))),
                        KeyValue::new("Most delayed", Cell::duration(sent.most_delayed)),
                    ],
                    DeviceClock::Absent => vec![arriving],
                };
                let stalling = warning_when(timing.stalls > 0);
                clocked
                    .into_iter()
                    .chain([
                        KeyValue::new("Stalls over 20 ms", Quantity::count(timing.stalls))
                            .level(stalling),
                        KeyValue::new("Longest gap", Cell::duration(timing.longest_gap))
                            .level(stalling),
                    ])
                    .collect()
            }
        };
        Section::dictionary(entries).titled("Controller reports, last 2 s")
    }

    fn kernel_error_section(&self) -> Option<Section> {
        (!self.kernel_errors.is_empty()).then(|| {
            let listed = self
                .kernel_errors
                .iter()
                .rev()
                .take(LISTED_KERNEL_ERRORS)
                .map(|error| {
                    KeyValue::new(
                        Cell::RelativeMoment(DateTime::<Utc>::from(error.at)),
                        error.message.clone(),
                    )
                    .level(Severity::Warning)
                });
            Section::dictionary(
                [KeyValue::new(
                    "Errors in the last 10 minutes",
                    Quantity::count(self.kernel_errors.len()),
                )
                .level(Severity::Warning)]
                .into_iter()
                .chain(listed),
            )
            .titled("Kernel messages")
        })
    }

    fn interference_sections(&self) -> Vec<Section> {
        match self.band_sharing {
            BandSharing::SharedWithWifi {
                network_name,
                frequency,
                wired_fallback,
            } if self.over_bluetooth() => {
                let ethernet = match wired_fallback {
                    WiredFallback::Connected => Some(KeyValue::new("Ethernet", "connected")),
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
                    Section::dictionary(
                        [
                            KeyValue::new("Wi-Fi network", network_name),
                            KeyValue::new("Frequency", Quantity::clock_megahertz(frequency)),
                            KeyValue::new(
                                "Channel",
                                two_point_four_gigahertz_channel(frequency).to_string(),
                            ),
                        ]
                        .into_iter()
                        .chain(ethernet),
                    )
                    .titled("Interference"),
                    Section::note("Shared radio band", Some(remedy.to_owned())),
                ]
            }
            _ if self.over_bluetooth() && self.unstable() => vec![Section::note(
                "Unstable Bluetooth link",
                Some(
                    "Move the controller closer to the Bluetooth adapter, or connect it by cable."
                        .to_owned(),
                ),
            )],
            _ => Vec::new(),
        }
    }
}

impl BarWidget for ControllerLinkReading<'_> {
    fn name(&self) -> &'static str {
        "Controller lag"
    }

    fn icon(&self) -> Glyph {
        if self.unstable() {
            Phosphor::LinkBreak(Weight::Regular).into()
        } else {
            Phosphor::Link(Weight::Regular).into()
        }
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self.report_timing {
            MeasuredReportTiming::Pending => None,
            MeasuredReportTiming::Measured(timing) => Some(match timing.device_clock {
                DeviceClock::Present(sent) => {
                    format!("{:.0}% lost", sent.lost_share.get::<percent>())
                }
                DeviceClock::Absent if timing.stalls > 0 => format!("{} stalls", timing.stalls),
                DeviceClock::Absent => gap_label(timing.longest_gap),
            }),
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.unstable() {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections(
            [Some(self.report_section()), self.kernel_error_section()]
                .into_iter()
                .flatten()
                .chain(self.interference_sections()),
        )
    }
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
        assert!(shares_bluetooth_band(at_megahertz(2437.0)));
        assert!(!shares_bluetooth_band(at_megahertz(5180.0)));
    }
}
