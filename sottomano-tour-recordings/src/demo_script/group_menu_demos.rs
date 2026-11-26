use std::time::Duration;

use sottomano_widget_scenes::{
    scenes::{group_menu_scenes, menu_tour_scenes},
    script_steps::{Step, sole_indicator_key},
};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic, PanelPin, PanelPinDuration},
    hover_cards::HoverTarget,
};

use crate::demo_script::Demo;

pub(super) const GROUP_MENU_ACCORDION_SEVERAL: Demo = Demo {
    name: "group_menu_accordion_several",
    scene: &group_menu_scenes::GROUP_MENU_ACCORDION_SEVERAL,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
        Step::PressText("Compute"),
        Step::PressText("Memory"),
    ],
};

const PINNED_CPU_TEMPERATURE: GroupMenuPin = GroupMenuPin::Item(sole_indicator_key(
    IndicatorGroupFamily::Compute,
    "CPU temperature",
));

const PINNED_CPU_TEMPERATURE_ON_PANEL: PanelPin = PanelPin {
    menu: GroupMenuTopic::Hardware,
    entry: PINNED_CPU_TEMPERATURE,
    duration: PanelPinDuration::Permanent,
};

const ALERTING_RAM_GROWTH: PanelPin = PanelPin {
    menu: GroupMenuTopic::Hardware,
    entry: GroupMenuPin::Item(sole_indicator_key(
        IndicatorGroupFamily::MemoryBoard,
        "RAM growth",
    )),
    duration: PanelPinDuration::UntilAlertEnds,
};

pub(super) const GROUP_MENU_PIN: Demo = Demo {
    name: "group_menu_pin",
    scene: &menu_tour_scenes::HARDWARE_MENU,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
        Step::HoverText("Compute"),
        Step::PressPin(PINNED_CPU_TEMPERATURE),
        Step::PressAway,
        Step::HoverPinned(PINNED_CPU_TEMPERATURE_ON_PANEL),
    ],
};

pub(super) const MEMORY_GROWTH_ALERT: Demo = Demo {
    name: "memory_growth_alert",
    scene: &menu_tour_scenes::HARDWARE_MENU_RUNAWAY_MEMORY,
    script: &[
        Step::RestFor(Duration::from_millis(3500)),
        Step::HoverPinned(ALERTING_RAM_GROWTH),
        Step::ReleasePinned(ALERTING_RAM_GROWTH),
        Step::RestFor(Duration::from_millis(750)),
    ],
};

pub(super) const GROUP_MENU_SETTINGS: Demo = Demo {
    name: "group_menu_settings",
    scene: &group_menu_scenes::GROUP_MENU_SETTINGS,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Settings)),
        Step::PressText("Panels"),
        Step::RestFor(Duration::from_millis(600)),
    ],
};

pub(super) const COMPUTE_CPU_PINNED: Demo = Demo {
    name: "compute_cpu_pinned",
    scene: &group_menu_scenes::COMPUTE_CPU_PINNED,
    script: &[
        Step::HoverFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute)),
        Step::PressFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute)),
    ],
};
