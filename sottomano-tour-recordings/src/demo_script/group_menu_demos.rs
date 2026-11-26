use std::time::Duration;

use sottomano_widget_scenes::{
    scenes::{group_menu_scenes, menu_tour_scenes},
    script_steps::{Step, indicator_row},
};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic},
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

pub(super) const GROUP_MENU_MEDIA: Demo = Demo {
    name: "group_menu_media",
    scene: &menu_tour_scenes::MEDIA_MENU,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
        Step::PressText("Spotify"),
        Step::PressText("Audio"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Audio,
            "Microphone volume",
        )),
    ],
};

pub(super) const GROUP_MENU_PIN: Demo = Demo {
    name: "group_menu_pin",
    scene: &menu_tour_scenes::HARDWARE_MENU,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
        Step::PressPin(GroupMenuPin::Group(IndicatorGroupIdentity::sole(
            IndicatorGroupFamily::MemoryBoard,
        ))),
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
