use sottomano_widgets::{
    clock::Clock, widget_groups::appearance_presets_group::AppearancePresetsGroup,
    widgets::appearance_presets::ShellAppearance,
};

use crate::showcased_group::{ShowcasedAppearancePresets, ShowcasedGroup};

pub fn group(_clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::AppearancePresets(Box::new(ShowcasedAppearancePresets {
        group: AppearancePresetsGroup::default(),
        appearance: ShellAppearance::default(),
    }))
}
