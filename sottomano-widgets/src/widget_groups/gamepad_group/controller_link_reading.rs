use std::time::Duration;

use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::{
    gamepad_controller_input::{
        accelerometer_report_rate::ReportRate, evdev_gamepad_devices::GamepadTransport,
    },
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const STUTTERING_REPORT_GAP: Duration = Duration::from_millis(50);
const LISTED_KERNEL_ERRORS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasuredReportRate {
    Pending,
    Measured(ReportRate),
}

fn gap_label(longest_gap: Duration) -> String {
    format!("{} ms", longest_gap.as_millis().max(1))
}

pub struct ControllerLinkReading<'a> {
    pub transport: &'a GamepadTransport,
    pub report_rate: MeasuredReportRate,
    pub kernel_errors: Vec<&'a DeviceKernelError>,
}

impl ControllerLinkReading<'_> {
    fn stuttering(&self) -> bool {
        matches!(self.report_rate, MeasuredReportRate::Measured(rate) if rate.longest_gap > STUTTERING_REPORT_GAP)
    }

    fn unstable(&self) -> bool {
        self.stuttering() || !self.kernel_errors.is_empty()
    }

    fn connection_section(&self) -> Section {
        let measured = match self.report_rate {
            MeasuredReportRate::Pending => vec![KeyValue::new("Reports", "measuring")],
            MeasuredReportRate::Measured(rate) => vec![
                KeyValue::new("Reports", format!("{} per second", rate.reports_per_second)),
                KeyValue::new("Longest gap", Cell::duration(rate.longest_gap)).level(
                    if self.stuttering() {
                        Severity::Warning
                    } else {
                        Severity::Normal
                    },
                ),
            ],
        };
        let kernel_error_count = (!self.kernel_errors.is_empty()).then(|| {
            KeyValue::new(
                "Kernel errors",
                format!("{} in the last 10 minutes", self.kernel_errors.len()),
            )
            .level(Severity::Warning)
        });
        Section::dictionary(measured.into_iter().chain(kernel_error_count))
    }

    fn kernel_error_section(&self, clock: Clock) -> Option<Section> {
        (!self.kernel_errors.is_empty()).then(|| {
            Section::dictionary(
                self.kernel_errors
                    .iter()
                    .rev()
                    .take(LISTED_KERNEL_ERRORS)
                    .map(|error| {
                        KeyValue::new(
                            Cell::duration(clock.wall.duration_since(error.at).unwrap_or_default()),
                            error.message.clone(),
                        )
                        .level(Severity::Warning)
                    }),
            )
            .titled("Kernel messages")
        })
    }

    fn remedy_section(&self) -> Option<Section> {
        match self.transport {
            GamepadTransport::Bluetooth { .. } if self.unstable() => Some(Section::note(
                "Unstable Bluetooth link",
                Some(
                    "Move the controller closer to the Bluetooth adapter, or connect it by cable."
                        .to_owned(),
                ),
            )),
            _ => None,
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
        match self.report_rate {
            MeasuredReportRate::Pending => None,
            MeasuredReportRate::Measured(rate) => Some(gap_label(rate.longest_gap)),
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.unstable() {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        TooltipBody::sections(
            [Some(self.connection_section())]
                .into_iter()
                .chain([self.kernel_error_section(clock), self.remedy_section()])
                .flatten(),
        )
    }
}
