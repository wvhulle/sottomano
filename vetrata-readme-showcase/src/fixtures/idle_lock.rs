use std::time::Duration;

use vetrata_shell::{
    state::idle::Timeouts,
    widget_groups::idle_timeouts_group::{IdleTimeoutsGroup, IdleTimeoutsMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub fn group() -> ShowcasedGroup {
    let mut group = IdleTimeoutsGroup::default();
    let _ = group.update(IdleTimeoutsMessage::Loaded(Timeouts {
        lock_user: Some(Duration::from_mins(15)),
        screen_off: Some(Duration::from_mins(20)),
        standby: Some(Duration::from_hours(1)),
        power_down: None,
    }));
    ShowcasedGroup::IdleTimeouts(Box::new(group))
}
