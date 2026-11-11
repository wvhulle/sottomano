use std::num::NonZeroUsize;

use chrono::{DateTime, Utc};
use sottomano_desktop_sources::{
    gamepad_controller_input::accelerometer_report_timing::{DeviceClock, ReportTiming},
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};
use sottomano_toml_config::severity::Severity;
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    hover_cards::hover_card_body::Section,
    widgets::{
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

const NOTICEABLE_LOSS_PERCENT: f64 = 5.0;
const LISTED_KERNEL_ERRORS: NonZeroUsize = NonZeroUsize::new(5).unwrap();

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeasuredReportTiming {
    Pending,
    Measured(ReportTiming),
}

pub struct KernelErrorListing<'a> {
    pub kernel_errors: &'a [&'a DeviceKernelError],
}

#[must_use]
pub fn loss_severity(lost_share: Ratio) -> Severity {
    if lost_share.get::<percent>() >= NOTICEABLE_LOSS_PERCENT {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportHealth {
    Degraded,
    Healthy,
}

impl MeasuredReportTiming {
    #[must_use]
    pub fn report_health(self) -> ReportHealth {
        match self {
            MeasuredReportTiming::Measured(timing)
                if timing.stalls > 0
                    || matches!(
                        timing.device_clock,
                        DeviceClock::Present(sent) if loss_severity(sent.lost_share) == Severity::Warning
                    ) =>
            {
                ReportHealth::Degraded
            }
            MeasuredReportTiming::Measured(_) | MeasuredReportTiming::Pending => {
                ReportHealth::Healthy
            }
        }
    }

    #[must_use]
    pub fn report_section(self, title: &'static str) -> Section {
        let entries = match self {
            MeasuredReportTiming::Pending => vec![LabeledValue::new("Reports", "measuring")],
            MeasuredReportTiming::Measured(timing) => {
                let arriving =
                    LabeledValue::new("Arriving", Quantity::refresh_hertz(timing.arrival_rate));
                let clocked = match timing.device_clock {
                    DeviceClock::Present(sent) => vec![
                        LabeledValue::new("Sent", Quantity::refresh_hertz(sent.sent_rate)),
                        arriving,
                        LabeledValue::new("Lost", Quantity::share(sent.lost_share))
                            .level(loss_severity(sent.lost_share)),
                        LabeledValue::new("Most delayed", ValueCell::duration(sent.most_delayed)),
                    ],
                    DeviceClock::Absent => vec![arriving],
                };
                let stalling = if timing.stalls > 0 {
                    Severity::Warning
                } else {
                    Severity::Normal
                };
                clocked
                    .into_iter()
                    .chain([
                        LabeledValue::new("Stalls over 20 ms", Quantity::count(timing.stalls))
                            .level(stalling),
                        LabeledValue::new("Longest gap", ValueCell::duration(timing.longest_gap))
                            .level(stalling),
                    ])
                    .collect()
            }
        };
        Section::dictionary(entries).titled(title)
    }
}

impl KernelErrorListing<'_> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kernel_errors.is_empty()
    }

    #[must_use]
    pub fn section(&self) -> Option<Section> {
        (!self.is_empty()).then(|| {
            let listed = self.kernel_errors.iter().rev().map(|error| {
                LabeledValue::new(
                    ValueCell::RelativeMoment(DateTime::<Utc>::from(error.at)),
                    error.message.clone(),
                )
                .level(Severity::Warning)
            });
            Section::dictionary(
                [LabeledValue::new(
                    "Errors in the last 10 minutes",
                    Quantity::count(self.kernel_errors.len()),
                )
                .level(Severity::Warning)]
                .into_iter()
                .chain(listed),
            )
            .truncated_to(LISTED_KERNEL_ERRORS)
            .titled("Kernel messages")
        })
    }
}
