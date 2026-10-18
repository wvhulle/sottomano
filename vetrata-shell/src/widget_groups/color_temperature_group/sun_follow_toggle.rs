use crate::widgets::{
    phosphor::{Phosphor, Weight},
    toggle_control::SwitchPresses,
    unit_switch::UnitSwitchFace,
};

pub const SUN_FOLLOW: UnitSwitchFace = UnitSwitchFace {
    name: "Sun follow",
    icon: Phosphor::SunHorizon(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "warm the screen at sunset and cool it at sunrise again",
        turning_off: "stop shifting screen colour with the sun; keep the current temperature",
    },
};
