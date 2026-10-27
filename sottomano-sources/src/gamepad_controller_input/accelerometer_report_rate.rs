use std::{
    future::ready,
    mem,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use evdev::{Device, EventSummary, PropType, SynchronizationCode};
use futures::{Stream, StreamExt, stream};

use super::evdev_gamepad_devices::RawGamepadEvent;
use crate::{
    delivery_cadence_drivers::PeriodicPolling, system_protocol_clients::sysfs_attribute_reads,
};

const REPORT_RATE_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportRate {
    pub reports_per_second: u32,
    pub longest_gap: Duration,
}

#[derive(Debug, Clone, Copy)]
enum LastReport {
    Never,
    At(SystemTime),
}

#[derive(Debug, Clone, Copy)]
pub struct OpenReportWindow {
    opened_at: SystemTime,
    last_report: LastReport,
    reports: u32,
    longest_gap: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ReportWindow {
    #[default]
    Unopened,
    Open(OpenReportWindow),
}

impl ReportWindow {
    pub fn record_report(&mut self, at: SystemTime) {
        if let ReportWindow::Open(window) = self {
            if let LastReport::At(last) = window.last_report {
                let gap = at.duration_since(last).unwrap_or_default();
                window.longest_gap = window.longest_gap.max(gap);
            }
            window.last_report = LastReport::At(at);
            window.reports += 1;
        }
    }

    pub fn closed(&mut self, now: SystemTime) -> Option<ReportRate> {
        let carried_last_report = match self {
            ReportWindow::Open(window) => window.last_report,
            ReportWindow::Unopened => LastReport::Never,
        };
        let previous = mem::replace(
            self,
            ReportWindow::Open(OpenReportWindow {
                opened_at: now,
                last_report: carried_last_report,
                reports: 0,
                longest_gap: Duration::ZERO,
            }),
        );
        let ReportWindow::Open(window) = previous else {
            return None;
        };
        let elapsed = now
            .duration_since(window.opened_at)
            .ok()
            .filter(|elapsed| !elapsed.is_zero())?;
        let silence_since_last_report = match window.last_report {
            LastReport::Never => elapsed,
            LastReport::At(last) => now.duration_since(last).unwrap_or_default(),
        };
        Some(ReportRate {
            reports_per_second: (f64::from(window.reports) / elapsed.as_secs_f64()).round() as u32,
            longest_gap: window.longest_gap.max(silence_since_last_report),
        })
    }
}

fn is_accelerometer(event_node: &Path) -> bool {
    Device::open(event_node)
        .is_ok_and(|device| device.properties().contains(PropType::ACCELEROMETER))
}

pub fn accelerometer_node_beside(slot: usize) -> Option<PathBuf> {
    sysfs_attribute_reads::list_dir(format!("/sys/class/input/event{slot}/device/device/input"))
        .into_iter()
        .flat_map(sysfs_attribute_reads::list_dir)
        .filter_map(|sibling| {
            let name = sibling.file_name()?.to_str()?;
            name.starts_with("event")
                .then(|| Path::new("/dev/input").join(name))
        })
        .find(|event_node| is_accelerometer(event_node))
}

enum MotionSample {
    Report(SystemTime),
    WindowEnd,
}

pub fn report_rates(
    slot: usize,
    accelerometer: &Path,
) -> impl Stream<Item = RawGamepadEvent> + Send + use<> {
    let opened = Device::open(accelerometer).and_then(Device::into_event_stream);
    let reports = match opened {
        Ok(events) => events
            .take_while(|event| ready(event.is_ok()))
            .filter_map(|event| ready(event.ok()))
            .filter_map(|event| {
                ready(match event.destructure() {
                    EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                        Some(MotionSample::Report(event.timestamp()))
                    }
                    _ => None,
                })
            }),
        Err(e) => {
            tracing::warn!("gamepad: cannot read {}: {e}", accelerometer.display());
            return stream::empty().right_stream();
        }
    };
    let window_ends = PeriodicPolling::every(REPORT_RATE_WINDOW)
        .poll_ticks()
        .map(|_| MotionSample::WindowEnd);
    stream::select(reports, window_ends)
        .scan(ReportWindow::default(), move |window, sample| {
            ready(Some(match sample {
                MotionSample::Report(at) => {
                    window.record_report(at);
                    None
                }
                MotionSample::WindowEnd => window
                    .closed(SystemTime::now())
                    .map(|rate| RawGamepadEvent::ReportRate { slot, rate }),
            }))
        })
        .filter_map(ready)
        .left_stream()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_millis(millis: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(millis)
    }

    #[test]
    fn the_first_window_end_only_opens_the_window() {
        assert_eq!(ReportWindow::default().closed(at_millis(0)), None);
    }

    #[test]
    fn steady_reports_give_their_rate_and_spacing() {
        let mut window = ReportWindow::default();
        window.closed(at_millis(0));
        (1..=500).for_each(|report| window.record_report(at_millis(report * 4)));
        assert_eq!(
            window.closed(at_millis(2000)),
            Some(ReportRate {
                reports_per_second: 250,
                longest_gap: Duration::from_millis(4),
            })
        );
    }

    #[test]
    fn a_dropout_shows_as_the_longest_gap_of_its_window_only() {
        let mut window = ReportWindow::default();
        window.closed(at_millis(0));
        window.record_report(at_millis(100));
        window.record_report(at_millis(400));
        window.record_report(at_millis(1990));
        let stuttering = window.closed(at_millis(2000)).unwrap();
        assert_eq!(stuttering.longest_gap, Duration::from_millis(1590));
        window.record_report(at_millis(2004));
        window.record_report(at_millis(3995));
        assert_eq!(
            window.closed(at_millis(4000)).unwrap().longest_gap,
            Duration::from_millis(1991)
        );
    }

    #[test]
    fn a_silent_controller_reports_the_whole_window_as_a_gap() {
        let mut window = ReportWindow::default();
        window.closed(at_millis(0));
        assert_eq!(
            window.closed(at_millis(2000)),
            Some(ReportRate {
                reports_per_second: 0,
                longest_gap: Duration::from_millis(2000),
            })
        );
    }
}
