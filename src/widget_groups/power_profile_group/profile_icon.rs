use crate::{
    data::source::power_profile::{PowerProfile, Profile},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::{FontAwesome, Solid},
        tooltip_body::TooltipBody,
    },
};

pub fn profile_face(profile: Profile) -> FontAwesome {
    match profile {
        Profile::PowerSaver => Solid::Leaf.into(),
        Profile::Balanced => Solid::ScaleBalanced.into(),
        Profile::Performance => Solid::Rocket.into(),
    }
}

impl BarWidget for PowerProfile {
    fn icon(&self) -> Glyph {
        profile_face(self.active_profile).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
    }
}
