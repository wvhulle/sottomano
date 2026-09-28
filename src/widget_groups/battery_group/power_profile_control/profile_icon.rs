use crate::{
    data::source::power_profile::Profile,
    widgets::font_awesome::{FontAwesome, Solid},
};

pub fn profile_face(profile: Profile) -> FontAwesome {
    match profile {
        Profile::PowerSaver => Solid::Leaf.into(),
        Profile::Balanced => Solid::ScaleBalanced.into(),
        Profile::Performance => Solid::Rocket.into(),
    }
}
