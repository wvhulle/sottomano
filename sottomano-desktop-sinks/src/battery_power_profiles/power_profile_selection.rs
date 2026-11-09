use sottomano_desktop_sources::battery_power_profiles::power_profiles_daemon::{self, Profile};

#[derive(Debug, thiserror::Error)]
pub enum ProfileSwitchError {
    #[error("power-profiles-daemon is not reachable on the system bus")]
    DaemonUnreachable,
    #[error("power-profiles-daemon call failed: {0}")]
    Call(#[from] zbus::Error),
}

pub async fn set(profile: Profile) -> Result<(), ProfileSwitchError> {
    let power_profiles = power_profiles_daemon::system_power_profiles()
        .await
        .map_err(|_| ProfileSwitchError::DaemonUnreachable)?;
    Ok(power_profiles.set_active_profile(profile.name()).await?)
}
