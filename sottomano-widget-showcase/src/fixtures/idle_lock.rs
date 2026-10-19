use std::time::Duration;

use sottomano_state::idle::Timeouts;
use sottomano_widgets::{
    clock::Clock,
    widget_groups::idle_timeouts_group::{IdleTimeoutsGroup, IdleTimeoutsMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(_clock: Clock) -> ShowcasedGroup {
    let mut group = IdleTimeoutsGroup::default();
    let _ = group.update(IdleTimeoutsMessage::Loaded(Timeouts {
        lock_user: Some(Duration::from_mins(15)),
        screen_off: Some(Duration::from_mins(20)),
        standby: Some(Duration::from_hours(1)),
        power_down: None,
    }));
    ShowcasedGroup::IdleTimeouts(Box::new(group))
}
