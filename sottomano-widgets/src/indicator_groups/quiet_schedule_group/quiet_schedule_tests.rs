use std::time::Duration;

use chrono::NaiveTime;

use super::{QuietChange, QuietScheduleGroup};
use crate::indicator_groups::quiet_schedule_group::{
    quiet_window::{QuietPhase, QuietScheduleAdherence},
    sqlite_quiet_hours::{QuietHoursSchedule, ScreenOffWhileQuiet},
};

fn group_in_phase(phase: QuietPhase, screen_off: ScreenOffWhileQuiet) -> QuietScheduleGroup {
    let mut group = QuietScheduleGroup::default();
    group.adopt_schedule(QuietHoursSchedule {
        screen_off,
        ..QuietHoursSchedule::default()
    });
    group.phase = phase;
    group
}

#[test]
fn a_loaded_schedule_reads_back_unchanged() {
    let loaded = QuietHoursSchedule {
        schedule_adherence: QuietScheduleAdherence::Ignored,
        start: NaiveTime::from_hms_opt(21, 45, 0).unwrap(),
        end: NaiveTime::from_hms_opt(6, 15, 0).unwrap(),
        screen_off: ScreenOffWhileQuiet::FollowsIdleTimer,
    };
    let mut quiet_schedule_group = QuietScheduleGroup::default();
    quiet_schedule_group.adopt_schedule(loaded);
    assert_eq!(quiet_schedule_group.schedule(), loaded);
}

#[test]
fn retiming_screen_off_while_quiet_overrides_the_idle_timer() {
    let group = group_in_phase(
        QuietPhase::Active,
        ScreenOffWhileQuiet::After(Duration::from_mins(1)),
    );
    let change = group.quiet_change_since(
        QuietPhase::Active,
        ScreenOffWhileQuiet::After(Duration::from_mins(10)),
    );
    assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
}

#[test]
fn retiming_screen_off_outside_quiet_hours_changes_nothing() {
    let group = group_in_phase(
        QuietPhase::Inactive,
        ScreenOffWhileQuiet::After(Duration::from_mins(1)),
    );
    let change = group.quiet_change_since(
        QuietPhase::Inactive,
        ScreenOffWhileQuiet::After(Duration::from_mins(10)),
    );
    assert_eq!(change, QuietChange::Unchanged);
}

#[test]
fn entering_quiet_hours_without_screen_off_follows_the_idle_timer() {
    let group = group_in_phase(QuietPhase::Active, ScreenOffWhileQuiet::FollowsIdleTimer);
    let change =
        group.quiet_change_since(QuietPhase::Inactive, ScreenOffWhileQuiet::FollowsIdleTimer);
    assert_eq!(change, QuietChange::ScreenOffFollowsIdleTimer);
}

#[test]
fn entering_quiet_hours_applies_the_screen_off_timeout() {
    let group = group_in_phase(
        QuietPhase::Active,
        ScreenOffWhileQuiet::After(Duration::from_mins(1)),
    );
    let change = group.quiet_change_since(
        QuietPhase::Inactive,
        ScreenOffWhileQuiet::After(Duration::from_mins(1)),
    );
    assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
}

#[test]
fn leaving_quiet_hours_ends_the_override() {
    let group = group_in_phase(
        QuietPhase::Inactive,
        ScreenOffWhileQuiet::After(Duration::from_mins(1)),
    );
    let change = group.quiet_change_since(
        QuietPhase::Active,
        ScreenOffWhileQuiet::After(Duration::from_mins(1)),
    );
    assert_eq!(change, QuietChange::Ended);
}
