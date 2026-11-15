use sottomano_desktop_sources::battery_power_profiles::power_profiles_daemon::{self, Profile};

use crate::sink_write_errors::BusWriteError;

pub async fn set(profile: Profile) -> Result<(), BusWriteError> {
    let power_profiles = power_profiles_daemon::system_power_profiles()
        .await
        .map_err(|_| BusWriteError::ServiceUnreachable("power-profiles-daemon"))?;
    Ok(power_profiles.set_active_profile(profile.name()).await?)
}
