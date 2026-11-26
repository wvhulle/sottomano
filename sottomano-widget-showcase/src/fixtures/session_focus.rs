use std::time::Duration;

use sottomano_desktop_sources::login_session_presence::{
    focus_time_ledger::{FocusProject, FocusSignal, Focused, ScreenEdge},
    logind_manager::SleepEdge,
    wayland_idle_notify::IdleEdge,
};
use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock, panel_menus::agenda_menu::session_focus_group::SessionFocusGroup,
};

use crate::showcased_group::ShowcasedGroup;

fn focused(window: u64, app_id: &str) -> FocusSignal {
    FocusSignal::WindowFocused(Focused {
        window,
        app_id: app_id.to_owned(),
        project: FocusProject::OutsideRepository,
    })
}

fn minutes_before(clock: Clock, minutes: u64) -> Clock {
    let earlier = Duration::from_mins(minutes);
    Clock {
        instant: clock
            .instant
            .checked_sub(earlier)
            .expect("the showcase clock runs long after boot"),
        wall: clock
            .wall
            .checked_sub(earlier)
            .expect("the showcase clock runs long after the epoch"),
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let focus_config = Config::default().indicator_groups.session_focus;
    let mut group = SessionFocusGroup::started_at(&focus_config, minutes_before(clock, 230).utc());
    for (signal, minutes_ago) in [
        (focused(1, "kitty"), 230),
        (FocusSignal::ScreenPower(ScreenEdge::Off), 200),
        (FocusSignal::ScreenPower(ScreenEdge::On), 186),
        (focused(1, "kitty"), 186),
        (FocusSignal::Sleep(SleepEdge::Suspending), 150),
        (FocusSignal::Sleep(SleepEdge::Resumed), 105),
        (focused(2, "firefox"), 105),
        (FocusSignal::Idle(IdleEdge::Idled), 50),
        (FocusSignal::Idle(IdleEdge::Resumed), 38),
        (focused(1, "kitty"), 38),
    ] {
        let _ = group.record_signal(signal, &focus_config, minutes_before(clock, minutes_ago));
    }
    ShowcasedGroup::SessionFocus(Box::new(group))
}
