use std::time::Duration;

use corredo::popovers::{
    NestedMenuLayout, NestedMenuOpenTrigger, NestedMenuPresentation, OpenGroupLimit,
    SubmenuConnector,
};
use sottomano_widgets::{
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic},
    hover_cards::HoverTarget,
    indicator_groups::indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
};

use crate::{
    demo_script::{Demo, DemoBackdrop, GroupShown, Step, UiPart, indicator_row},
    fixtures::{audio, compute, group_menu_members, shell_settings, unchanging},
};

pub(super) const GROUP_MENU_ACCORDION_SEVERAL: Demo = Demo {
    name: "group_menu_accordion_several",
    parts: &[UiPart::GroupMenus],
    group: group_menu_members::hardware_group_menu,
    live: group_menu_members::hardware_menu_live,
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenuPresented {
        menu: GroupMenuTopic::Hardware,
        presentation: NestedMenuPresentation {
            layout: NestedMenuLayout::Accordion,
            open_trigger: NestedMenuOpenTrigger::Press,
            accordion_open_limit: OpenGroupLimit::Several,
            submenu_connector: SubmenuConnector::Hidden,
        },
    },
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
        Step::PressText("Compute"),
        Step::PressText("Memory"),
    ],
};

pub(super) const GROUP_MENU_MEDIA: Demo = Demo {
    name: "group_menu_media",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: group_menu_members::media_group_menu,
    live: audio::live,
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Media),
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
    parts: &[UiPart::GroupMenus, UiPart::PanelPins],
    group: group_menu_members::hardware_group_menu,
    live: group_menu_members::hardware_menu_live,
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
        Step::PressPin(GroupMenuPin::Group(IndicatorGroupIdentity::sole(
            IndicatorGroupFamily::MemoryBoard,
        ))),
        Step::RestFor(Duration::from_millis(750)),
    ],
};

pub(super) const GROUP_MENU_APPEARANCE: Demo = Demo {
    name: "group_menu_appearance",
    parts: &[UiPart::GroupMenus],
    group: |_| shell_settings::sharing_a_unit(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Appearance),
    script: &[Step::Press(HoverTarget::GroupMenu(
        GroupMenuTopic::Appearance,
    ))],
};

pub(super) const COMPUTE_CPU_PINNED: Demo = Demo {
    name: "compute_cpu_pinned",
    parts: &[UiPart::HoverCards, UiPart::PanelFaces, UiPart::PanelPins],
    group: compute::group,
    live: compute::live,
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::PinnedFrom {
        menu: GroupMenuTopic::Hardware,
        group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute),
    },
    script: &[
        Step::HoverFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute)),
        Step::PressFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute)),
    ],
};
