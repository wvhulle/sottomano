use sottomano_widgets::{
    clock::Clock, widget_groups::appearance_presets_group::AppearancePresetsMenu,
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(_clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::AppearancePresets(AppearancePresetsMenu)
}
