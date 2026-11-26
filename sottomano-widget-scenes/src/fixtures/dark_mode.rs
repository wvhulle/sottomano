use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::DeliveryCadence,
    screen_brightness_color::{solar_switch_schedule::ColorMode, umbra_dark_mode::DarkMode},
};
use sottomano_widgets::{
    clock::Clock,
    group_indicators::toggle_control::ToggleMessage,
    panel_menus::display_menu::dark_mode_group::{DarkModeGroup, DarkModeMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut dark_mode_group = DarkModeGroup::new(clock);
    let _ = dark_mode_group.update(
        DarkModeMessage::ModeToggle(ToggleMessage::Sample(SnapshotDelivery::fixture_delivered(
            Ok(DarkMode {
                color_mode: ColorMode::Dark,
            }),
            DeliveryCadence::LivePush,
        ))),
        clock,
    );
    ShowcasedGroup::DarkMode(Box::new(dark_mode_group))
}
