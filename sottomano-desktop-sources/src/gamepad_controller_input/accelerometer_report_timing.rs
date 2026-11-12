use std::{
    future::ready,
    mem,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use evdev::{Device, EventSummary, MiscCode, PropType, SynchronizationCode};
use futures::{Stream, StreamExt, stream};
use uom::si::{
    f64::{Frequency, Ratio},
    frequency::hertz,
    ratio::ratio,
};

use super::evdev_gamepad_devices::RawGamepadEvent;
use crate::{
    delivery_cadence_drivers::PeriodicPolling, system_protocol_clients::sysfs_attribute_reads,
};

const REPORT_TIMING_WINDOW: Duration = Duration::from_secs(2);
const STALL_GAP: Duration = Duration::from_millis(20);
const MISSED_REPORT_FACTOR: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SentReports {
    pub sent_rate: Frequency,
    pub lost_share: Ratio,
    pub most_delayed: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceClock {
    Absent,
    Present(SentReports),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReportTiming {
    pub arrival_rate: Frequency,
    pub stalls: u32,
    pub longest_gap: Duration,
    pub device_clock: DeviceClock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStamp {
    Absent,
    Micros(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionReport {
    pub arrived_at: SystemTime,
    pub stamp: DeviceStamp,
}

#[derive(Debug, Clone, Copy)]
enum CarriedReport {
    Never,
    Last(MotionReport),
}

#[derive(Debug, Clone)]
pub struct OpenReportWindow {
    opened_at: SystemTime,
    carried: CarriedReport,
    reports: Vec<MotionReport>,
}

#[derive(Debug, Clone, Default)]
pub enum ReportWindow {
    #[default]
    Unopened,
    Open(OpenReportWindow),
}

fn report_rate(count: usize, elapsed: Duration) -> Frequency {
    Frequency::new::<hertz>(count as f64 / elapsed.as_secs_f64())
}

fn lost_share(missed: usize, sent: usize) -> Ratio {
    if sent == 0 {
        Ratio::new::<ratio>(0.0)
    } else {
        Ratio::new::<ratio>(missed as f64 / sent as f64)
    }
}

fn elapsed_between(earlier: SystemTime, later: SystemTime) -> Duration {
    later.duration_since(earlier).unwrap_or_else(|clock_step| {
        tracing::debug!("gamepad report timing: the wall clock stepped back: {clock_step}");
        Duration::ZERO
    })
}

fn host_gaps(arrivals: &[SystemTime], silence_until: SystemTime) -> Vec<Duration> {
    arrivals
        .windows(2)
        .map(|pair| elapsed_between(pair[0], pair[1]))
        .chain(
            arrivals
                .last()
                .map(|last| elapsed_between(*last, silence_until)),
        )
        .collect()
}

#[expect(
    clippy::disallowed_methods,
    reason = "the controller stamps reports with a u32 microsecond clock that wraps around"
)]
fn device_gaps(stamps: &[u32]) -> Vec<u32> {
    stamps
        .windows(2)
        .map(|pair| pair[1].wrapping_sub(pair[0]))
        .collect()
}

fn median_gap(gaps: &[u32]) -> Option<u32> {
    let mut sorted = gaps.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len() / 2).copied()
}

fn missed_reports(gaps: &[u32], nominal_gap: u32) -> usize {
    gaps.iter()
        .filter(|gap| f64::from(**gap) > MISSED_REPORT_FACTOR * f64::from(nominal_gap))
        .map(|gap| (f64::from(*gap) / f64::from(nominal_gap)).round() as usize - 1)
        .sum()
}

#[expect(
    clippy::disallowed_methods,
    reason = "the controller stamps reports with a u32 microsecond clock that wraps around"
)]
fn arrival_spread(reports: &[(SystemTime, u32)]) -> Duration {
    let Some((first_arrival, first_stamp)) = reports.first() else {
        return Duration::ZERO;
    };
    let offsets: Vec<i128> = reports
        .iter()
        .scan(0_i128, |device_elapsed, (arrived_at, stamp)| {
            *device_elapsed = i128::from(stamp.wrapping_sub(*first_stamp));
            let host_elapsed = elapsed_between(*first_arrival, *arrived_at).as_micros();
            let host_elapsed = i128::try_from(host_elapsed)
                .expect("report windows last far less than i128 micros");
            Some(host_elapsed - *device_elapsed)
        })
        .collect();
    let spread =
        offsets.iter().max().copied().unwrap_or(0) - offsets.iter().min().copied().unwrap_or(0);
    Duration::from_micros(
        u64::try_from(spread)
            .expect("a spread between the largest and smallest offset is not negative and short"),
    )
}

fn sent_reports(sequence: &[MotionReport], arrived: usize, elapsed: Duration) -> DeviceClock {
    let stamped: Vec<(SystemTime, u32)> = sequence
        .iter()
        .map_while(|report| match report.stamp {
            DeviceStamp::Micros(stamp) => Some((report.arrived_at, stamp)),
            DeviceStamp::Absent => None,
        })
        .collect();
    if stamped.len() != sequence.len() || stamped.len() < 2 {
        return DeviceClock::Absent;
    }
    let stamps: Vec<u32> = stamped.iter().map(|(_, stamp)| *stamp).collect();
    let gaps = device_gaps(&stamps);
    let Some(nominal_gap) = median_gap(&gaps).filter(|gap| *gap > 0) else {
        return DeviceClock::Absent;
    };
    let missed = missed_reports(&gaps, nominal_gap);
    let sent = arrived + missed;
    DeviceClock::Present(SentReports {
        sent_rate: report_rate(sent, elapsed),
        lost_share: lost_share(missed, sent),
        most_delayed: arrival_spread(&stamped),
    })
}

impl ReportWindow {
    pub fn record_report(&mut self, report: MotionReport) {
        if let ReportWindow::Open(window) = self {
            window.reports.push(report);
        }
    }

    pub fn closed(&mut self, now: SystemTime) -> Option<ReportTiming> {
        let carried = match self {
            ReportWindow::Open(window) => window
                .reports
                .last()
                .copied()
                .map_or(window.carried, CarriedReport::Last),
            ReportWindow::Unopened => CarriedReport::Never,
        };
        let previous = mem::replace(
            self,
            ReportWindow::Open(OpenReportWindow {
                opened_at: now,
                carried,
                reports: Vec::new(),
            }),
        );
        let ReportWindow::Open(window) = previous else {
            return None;
        };
        let elapsed = now
            .duration_since(window.opened_at)
            .inspect_err(|clock_skew| {
                tracing::debug!("gamepad: report window closed before it opened: {clock_skew}");
            })
            .ok()
            .filter(|elapsed| !elapsed.is_zero())?;
        let sequence: Vec<MotionReport> = match window.carried {
            CarriedReport::Last(report) => [report]
                .into_iter()
                .chain(window.reports.iter().copied())
                .collect(),
            CarriedReport::Never => window.reports.clone(),
        };
        let leading_silence_start = match window.carried {
            CarriedReport::Last(_) => None,
            CarriedReport::Never => Some(window.opened_at),
        };
        let arrivals: Vec<SystemTime> = leading_silence_start
            .into_iter()
            .chain(sequence.iter().map(|report| report.arrived_at))
            .collect();
        let gaps = host_gaps(&arrivals, now);
        Some(ReportTiming {
            arrival_rate: report_rate(window.reports.len(), elapsed),
            stalls: u32::try_from(gaps.iter().filter(|gap| **gap > STALL_GAP).count())
                .expect("a report window holds fewer than u32::MAX gaps"),
            longest_gap: gaps.iter().max().copied().unwrap_or_default(),
            device_clock: sent_reports(&sequence, window.reports.len(), elapsed),
        })
    }
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
        .find(|event_node| {
            Device::open(event_node)
                .is_ok_and(|device| device.properties().contains(PropType::ACCELEROMETER))
        })
}

enum MotionSample {
    Report(MotionReport),
    WindowEnd,
}

pub fn report_timings(
    slot: usize,
    accelerometer: &Path,
) -> impl Stream<Item = RawGamepadEvent> + Send + use<> {
    let opened_events = Device::open(accelerometer)
        .and_then(Device::into_event_stream)
        .inspect_err(|open_failure| {
            tracing::warn!(
                "gamepad: cannot read {}: {open_failure}",
                accelerometer.display()
            );
        })
        .ok();
    let accelerometer_path = accelerometer.to_owned();
    stream::iter(opened_events).flat_map(move |events| {
        let accelerometer_path = accelerometer_path.clone();
        let reports = events
            .scan((), move |(), event| {
                ready(
                    event
                        .inspect_err(|read_failure| {
                            tracing::debug!(
                                "gamepad: events of {} ended: {read_failure}",
                                accelerometer_path.display()
                            );
                        })
                        .ok(),
                )
            })
            .scan(DeviceStamp::Absent, |pending_stamp, event| {
                ready(Some(match event.destructure() {
                    EventSummary::Misc(_, MiscCode::MSC_TIMESTAMP, value) => {
                        *pending_stamp = DeviceStamp::Micros(value.cast_unsigned());
                        None
                    }
                    EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                        Some(MotionSample::Report(MotionReport {
                            arrived_at: event.timestamp(),
                            stamp: mem::replace(pending_stamp, DeviceStamp::Absent),
                        }))
                    }
                    _ => None,
                }))
            })
            .filter_map(ready);
        let window_ends = PeriodicPolling::every(REPORT_TIMING_WINDOW)
            .poll_ticks()
            .map(|_| MotionSample::WindowEnd);
        stream::select(reports, window_ends)
            .scan(ReportWindow::default(), move |window, sample| {
                ready(Some(match sample {
                    MotionSample::Report(report) => {
                        window.record_report(report);
                        None
                    }
                    MotionSample::WindowEnd => window
                        .closed(SystemTime::now())
                        .map(|timing| RawGamepadEvent::ReportTiming { slot, timing }),
                }))
            })
            .filter_map(ready)
    })
}

#[cfg(test)]
mod tests {
    use uom::si::ratio::percent;

    use super::*;

    fn at_micros(micros: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_micros(micros)
    }

    fn stamped_report(arrived_micros: u64, device_micros: u32) -> MotionReport {
        MotionReport {
            arrived_at: at_micros(arrived_micros),
            stamp: DeviceStamp::Micros(device_micros),
        }
    }

    fn opened_window() -> ReportWindow {
        let mut window = ReportWindow::default();
        window.closed(at_micros(0));
        window
    }

    #[test]
    fn the_first_window_end_only_opens_the_window() {
        assert_eq!(ReportWindow::default().closed(at_micros(0)), None);
    }

    #[test]
    fn a_steady_wired_stream_loses_nothing() {
        let mut window = opened_window();
        (1..=500).for_each(|report| {
            window.record_report(stamped_report(report * 4000, (report * 4000) as u32));
        });
        let timing = window.closed(at_micros(2_000_000)).unwrap();
        assert_eq!(timing.arrival_rate.get::<hertz>().round() as i64, 250);
        assert_eq!(timing.stalls, 0);
        assert_eq!(timing.longest_gap, Duration::from_millis(4));
        let DeviceClock::Present(sent) = timing.device_clock else {
            panic!("expected a device clock");
        };
        assert_eq!(sent.sent_rate.get::<hertz>().round() as i64, 250);
        assert_eq!(sent.lost_share.get::<percent>().round() as i64, 0);
        assert_eq!(sent.most_delayed, Duration::ZERO);
    }

    #[test]
    fn bursts_of_dropped_reports_count_as_lost() {
        let mut window = opened_window();
        (1..=500)
            .filter(|report| report % 10 < 6)
            .for_each(|report| {
                window.record_report(stamped_report(report * 4000, (report * 4000) as u32));
            });
        let DeviceClock::Present(sent) = window.closed(at_micros(2_000_000)).unwrap().device_clock
        else {
            panic!("expected a device clock");
        };
        assert_eq!(sent.lost_share.get::<percent>().round() as i64, 40);
        assert_eq!(sent.sent_rate.get::<hertz>().round() as i64, 250);
    }

    #[test]
    fn a_long_host_stall_counts_once() {
        let mut window = opened_window();
        window.record_report(stamped_report(100_000, 100_000));
        window.record_report(stamped_report(130_000, 104_000));
        window.record_report(stamped_report(1_990_000, 108_000));
        let timing = window.closed(at_micros(2_000_000)).unwrap();
        assert_eq!(timing.stalls, 3);
        assert_eq!(timing.longest_gap, Duration::from_millis(1860));
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "the controller stamps reports with a u32 microsecond clock that wraps around"
    )]
    fn the_device_clock_may_wrap_around() {
        let mut window = opened_window();
        let start = u32::MAX - 4000;
        (0..10u64).for_each(|report| {
            window.record_report(stamped_report(
                1000 + report * 4000,
                start.wrapping_add((report * 4000) as u32),
            ));
        });
        let DeviceClock::Present(sent) = window.closed(at_micros(40_000)).unwrap().device_clock
        else {
            panic!("expected a device clock");
        };
        assert_eq!(sent.lost_share.get::<percent>().round() as i64, 0);
    }

    #[test]
    fn reports_held_back_and_released_in_a_burst_show_as_delay() {
        let mut window = opened_window();
        window.record_report(stamped_report(4000, 4000));
        window.record_report(stamped_report(30_000, 8000));
        window.record_report(stamped_report(30_100, 12_000));
        window.record_report(stamped_report(30_200, 16_000));
        let DeviceClock::Present(sent) = window.closed(at_micros(32_000)).unwrap().device_clock
        else {
            panic!("expected a device clock");
        };
        assert_eq!(sent.lost_share.get::<percent>().round() as i64, 0);
        assert_eq!(sent.most_delayed, Duration::from_millis(22));
    }

    #[test]
    fn reports_without_a_device_clock_only_give_arrival_figures() {
        let mut window = opened_window();
        window.record_report(MotionReport {
            arrived_at: at_micros(4000),
            stamp: DeviceStamp::Absent,
        });
        assert_eq!(
            window.closed(at_micros(8000)).unwrap().device_clock,
            DeviceClock::Absent
        );
    }
}
