use super::{GroupCollapsing, PanelGroupFraming};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    indicator_groups::{
        sqlite_singleton_flag::{SingletonFlag, SingletonFlagState},
        window_group::{WindowRowLayout, WindowTitleLength},
    },
};

impl From<PanelGroupFraming> for SwitchPosition {
    fn from(group_framing: PanelGroupFraming) -> Self {
        match group_framing {
            PanelGroupFraming::Disconnected => SwitchPosition::On,
            PanelGroupFraming::Joined => SwitchPosition::Off,
        }
    }
}

impl From<GroupCollapsing> for SwitchPosition {
    fn from(group_collapsing: GroupCollapsing) -> Self {
        match group_collapsing {
            GroupCollapsing::StayExpanded => SwitchPosition::On,
            GroupCollapsing::OnFocusLoss => SwitchPosition::Off,
        }
    }
}

impl SingletonFlagState for WindowRowLayout {
    const STORED_FLAG: SingletonFlag = SingletonFlag::WindowsGroupedByLocation;
    const STORED_AS_SET: Self = WindowRowLayout::GroupedByLocation;
    const STORED_AS_CLEARED: Self = WindowRowLayout::Flat;
}

impl From<WindowRowLayout> for SwitchPosition {
    fn from(window_row_layout: WindowRowLayout) -> Self {
        match window_row_layout {
            WindowRowLayout::GroupedByLocation => SwitchPosition::On,
            WindowRowLayout::Flat => SwitchPosition::Off,
        }
    }
}

impl SingletonFlagState for WindowTitleLength {
    const STORED_FLAG: SingletonFlag = SingletonFlag::WindowTitlesCapped;
    const STORED_AS_SET: Self = WindowTitleLength::Capped;
    const STORED_AS_CLEARED: Self = WindowTitleLength::Full;
}

impl From<WindowTitleLength> for SwitchPosition {
    fn from(window_title_length: WindowTitleLength) -> Self {
        match window_title_length {
            WindowTitleLength::Capped => SwitchPosition::On,
            WindowTitleLength::Full => SwitchPosition::Off,
        }
    }
}

pub(super) fn disconnected_toggle(group_framing: PanelGroupFraming) -> IconToggle {
    IconToggle::new(
        "Disconnected groups",
        Phosphor::Pill(Weight::Regular),
        group_framing,
        SwitchPresses {
            turning_on: "draw each group in its own frame",
            turning_off: "join the groups into one panel",
        },
    )
}

pub(super) fn pinned_groups_toggle(group_collapsing: GroupCollapsing) -> IconToggle {
    IconToggle::new(
        "Stay expanded",
        Phosphor::PushPin(Weight::Regular),
        group_collapsing,
        SwitchPresses {
            turning_on: "keep groups open until their handle is clicked",
            turning_off: "collapse groups when another window takes focus",
        },
    )
}

pub(super) fn window_row_layout_toggle(window_row_layout: WindowRowLayout) -> IconToggle {
    IconToggle::new(
        "Group windows by directory",
        Phosphor::Folders(Weight::Regular),
        window_row_layout,
        SwitchPresses {
            turning_on: "enclose neighbouring windows from one directory under one heading",
            turning_off: "show every window on its own with its directory",
        },
    )
}

pub(super) fn window_title_length_toggle(window_title_length: WindowTitleLength) -> IconToggle {
    IconToggle::new(
        "Cap window titles",
        Phosphor::Scissors(Weight::Regular),
        window_title_length,
        SwitchPresses {
            turning_on: "cut long window titles off at the configured width",
            turning_off: "show every window title in full",
        },
    )
}
