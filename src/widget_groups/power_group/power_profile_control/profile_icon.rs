use crate::{
    data::source::power_profile::Profile,
    widgets::phosphor::{Phosphor, Weight},
};

pub fn profile_face(profile: Profile) -> Phosphor {
    match profile {
        Profile::PowerSaver => Phosphor::Leaf(Weight::Regular),
        Profile::Balanced => Phosphor::Scales(Weight::Regular),
        Profile::Performance => Phosphor::Rocket(Weight::Regular),
    }
}
