use sottomano_desktop_sources::battery_power_profiles::power_profiles_daemon::{self, Profile};

#[must_use]
pub fn set(profile: Profile) -> bool {
    power_profiles_daemon::power_profiles_proxy()
        .is_some_and(|power_profiles| power_profiles.set_active_profile(profile.name()).is_ok())
}
