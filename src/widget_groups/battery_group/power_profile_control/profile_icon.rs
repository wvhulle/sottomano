use crate::{
    data::source::power_profile::Profile,
    widgets::phosphor::{self, Phosphor},
};

pub fn profile_face(profile: Profile) -> Phosphor {
    match profile {
        Profile::PowerSaver => phosphor::Leaf::Regular.into(),
        Profile::Balanced => phosphor::Scales::Regular.into(),
        Profile::Performance => phosphor::Rocket::Regular.into(),
    }
}
