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
    let DeviceClock::Present(sent) = window.closed(at_micros(40_000)).unwrap().device_clock else {
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
    let DeviceClock::Present(sent) = window.closed(at_micros(32_000)).unwrap().device_clock else {
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
