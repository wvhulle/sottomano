use sottomano_widgets::{
    clock::Clock, widget_groups::appearance_presets_group::AppearancePresetsGroup,
};

use crate::showcased_group::{ShowcasedAppearancePresets, ShowcasedGroup, recorded_look};

pub fn group(_clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::AppearancePresets(Box::new(ShowcasedAppearancePresets {
        group: AppearancePresetsGroup::default(),
        appearance: recorded_look(),
    }))
}
