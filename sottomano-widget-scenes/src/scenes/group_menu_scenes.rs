use corredo::popovers::{
    NestedMenuLayout, NestedMenuOpenTrigger, NestedMenuPresentation, OpenGroupLimit,
    SubmenuConnector,
};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    group_menus::group_menu_state::GroupMenuTopic,
};

use crate::{
    fixtures::{compute, group_menu_members, shell_settings, unchanging},
    scenes::{GroupShown, Scene, SceneBackdrop},
};

pub static GROUP_MENU_ACCORDION_SEVERAL: Scene = Scene {
    name: "group_menu_accordion_several",
    group: group_menu_members::hardware_group_menu,
    live: group_menu_members::hardware_menu_live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenuPresented {
        menu: GroupMenuTopic::Hardware,
        presentation: NestedMenuPresentation {
            layout: NestedMenuLayout::Accordion,
            open_trigger: NestedMenuOpenTrigger::Press,
            accordion_open_limit: OpenGroupLimit::Several,
            submenu_connector: SubmenuConnector::Hidden,
        },
    },
};

pub static GROUP_MENU_SETTINGS: Scene = Scene {
    name: "group_menu_settings",
    group: |_| shell_settings::surface_settings(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Settings),
};

pub static COMPUTE_CPU_PINNED: Scene = Scene {
    name: "compute_cpu_pinned",
    group: compute::group,
    live: compute::live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::PinnedFrom {
        menu: GroupMenuTopic::Hardware,
        group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute),
    },
};
