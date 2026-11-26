pub mod session_power_group;

use self::session_power_group::SessionPowerGroup;

#[derive(Default)]
pub struct SessionPowerMenu {
    pub session_power_group: SessionPowerGroup,
}
