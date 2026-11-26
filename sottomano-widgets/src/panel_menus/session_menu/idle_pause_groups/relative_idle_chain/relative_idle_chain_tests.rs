use std::time::Duration;

use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction, session_power_actions::SessionAction,
};

use super::{
    ChainBase, HibernateDelayDemand, ResolvedIdleChain, ScheduledAfterIdle, StageSchedule,
    StageSetting, SuspendThenHibernateSupport,
};

fn minutes(count: u64) -> StageSetting {
    StageSetting::After(Duration::from_mins(count))
}

fn resolved(settings: [StageSetting; 5]) -> ResolvedIdleChain {
    ResolvedIdleChain::resolve(settings, SuspendThenHibernateSupport::Permitted)
}

#[test]
fn each_offset_adds_to_the_previous_armed_stage() {
    let chain = resolved([
        minutes(5),
        minutes(2),
        minutes(20),
        StageSetting::Off,
        StageSetting::Off,
    ]);
    let triggers = chain.armed().triggers;
    let fire_times: Vec<Duration> = triggers.iter().map(|trigger| trigger.after).collect();
    assert_eq!(
        fire_times,
        [
            Duration::from_mins(5),
            Duration::from_mins(7),
            Duration::from_mins(27)
        ]
    );
    assert_eq!(
        chain.stage(IdleAction::Sleep).counted_from,
        ChainBase::Stage(IdleAction::ScreenOff)
    );
}

#[test]
fn a_disabled_stage_is_skipped_as_a_base() {
    let chain = resolved([
        StageSetting::Off,
        minutes(10),
        StageSetting::Off,
        StageSetting::Off,
        StageSetting::Off,
    ]);
    assert_eq!(
        chain.stage(IdleAction::ScreenOff).counted_from,
        ChainBase::LastInput
    );
    assert_eq!(
        chain.scheduled(SessionAction::ScreenOff),
        ScheduledAfterIdle::After(Duration::from_mins(10))
    );
}

#[test]
fn hibernate_after_standby_counts_time_asleep_and_standby_suspends_then_hibernates() {
    let chain = resolved([
        StageSetting::Off,
        StageSetting::Off,
        minutes(30),
        minutes(180),
        StageSetting::Off,
    ]);
    assert_eq!(
        chain.stage(IdleAction::Hibernate).schedule,
        StageSchedule::AsleepAfterStandby {
            asleep_for: Duration::from_mins(180)
        }
    );
    assert_eq!(
        chain.armed().standby_performs,
        SessionAction::SuspendThenHibernate
    );
    assert_eq!(
        chain.hibernate_delay_demand(),
        HibernateDelayDemand::Wanted(Duration::from_mins(180))
    );
    assert_eq!(chain.armed().triggers.len(), 1);
}

#[test]
fn standby_without_hibernate_is_a_plain_suspend() {
    let chain = resolved([
        StageSetting::Off,
        StageSetting::Off,
        minutes(30),
        StageSetting::Off,
        StageSetting::Off,
    ]);
    assert_eq!(chain.armed().standby_performs, SessionAction::Suspend);
    assert_eq!(
        chain.hibernate_delay_demand(),
        HibernateDelayDemand::Indifferent
    );
}

#[test]
fn zero_time_asleep_hibernates_straight_away() {
    let chain = resolved([
        StageSetting::Off,
        StageSetting::Off,
        minutes(30),
        StageSetting::After(Duration::ZERO),
        StageSetting::Off,
    ]);
    assert_eq!(chain.armed().standby_performs, SessionAction::Hibernate);
    assert_eq!(
        chain.hibernate_delay_demand(),
        HibernateDelayDemand::Indifferent
    );
}

#[test]
fn hibernate_after_standby_is_unavailable_without_suspend_then_hibernate() {
    let chain = ResolvedIdleChain::resolve(
        [
            StageSetting::Off,
            StageSetting::Off,
            minutes(30),
            minutes(60),
            StageSetting::Off,
        ],
        SuspendThenHibernateSupport::Refused,
    );
    assert_eq!(
        chain.stage(IdleAction::Hibernate).schedule,
        StageSchedule::Unavailable
    );
    assert_eq!(chain.armed().standby_performs, SessionAction::Suspend);
}

#[test]
fn zero_offset_from_last_input_is_never_armed() {
    let chain = resolved([
        StageSetting::After(Duration::ZERO),
        minutes(5),
        StageSetting::Off,
        StageSetting::Off,
        StageSetting::Off,
    ]);
    assert_eq!(
        chain.stage(IdleAction::Lock).schedule,
        StageSchedule::ZeroWithoutEarlierStage
    );
    assert_eq!(
        chain.stage(IdleAction::ScreenOff).counted_from,
        ChainBase::LastInput
    );
    assert_eq!(chain.armed().triggers.len(), 1);
}

#[test]
fn power_down_is_unreachable_once_the_machine_sleeps() {
    let chain = resolved([
        StageSetting::Off,
        StageSetting::Off,
        minutes(30),
        StageSetting::Off,
        minutes(60),
    ]);
    assert_eq!(
        chain.stage(IdleAction::PowerOff).schedule,
        StageSchedule::UnreachableWhileAsleep
    );
    assert_eq!(
        chain.scheduled(SessionAction::PowerOff),
        ScheduledAfterIdle::Disabled
    );
}

#[test]
fn without_standby_hibernate_and_power_down_follow_idle() {
    let chain = resolved([
        minutes(5),
        StageSetting::Off,
        StageSetting::Off,
        minutes(60),
        StageSetting::Off,
    ]);
    assert_eq!(
        chain.scheduled(SessionAction::Hibernate),
        ScheduledAfterIdle::After(Duration::from_mins(65))
    );
    let chain = resolved([
        minutes(5),
        StageSetting::Off,
        StageSetting::Off,
        StageSetting::Off,
        minutes(60),
    ]);
    assert_eq!(
        chain.scheduled(SessionAction::PowerOff),
        ScheduledAfterIdle::After(Duration::from_mins(65))
    );
}

#[test]
fn logind_sleep_is_never_scheduled_by_the_chain() {
    let chain = resolved([
        StageSetting::Off,
        StageSetting::Off,
        minutes(30),
        StageSetting::Off,
        StageSetting::Off,
    ]);
    assert_eq!(
        chain.scheduled(SessionAction::Sleep),
        ScheduledAfterIdle::Never
    );
    assert_eq!(
        chain.scheduled(SessionAction::Suspend),
        ScheduledAfterIdle::After(Duration::from_mins(30))
    );
}
