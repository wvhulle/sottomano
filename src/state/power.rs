//! Per-profile discharge-rate filter state (Kalman rate + variance), keyed by
//! battery serial and profile so time-to-depletion estimates survive restarts.
//! Values are stored as text like the other state tables.

use std::collections::HashMap;

use crate::{
    data::source::power_profile::Profile,
    state::{BoxError, open, text},
};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS power_profile (
    battery TEXT NOT NULL,
    profile TEXT NOT NULL,
    rate_w TEXT NOT NULL,
    variance TEXT NOT NULL,
    PRIMARY KEY (battery, profile)
)";

#[derive(Debug, Clone, Copy)]
pub struct RateState {
    pub rate_w: f64,
    pub variance: f64,
}

pub async fn load(battery: String) -> HashMap<Profile, RateState> {
    try_load(&battery).await.unwrap_or_else(|e| {
        eprintln!("power profile: load failed: {e}");
        HashMap::new()
    })
}

pub async fn save(battery: String, profile: Profile, state: RateState) {
    if let Err(e) = try_save(&battery, profile, state).await {
        eprintln!("power profile: save failed: {e}");
    }
}

async fn try_load(battery: &str) -> Result<HashMap<Profile, RateState>, BoxError> {
    let conn = open().await?;
    let mut rows = conn
        .query(
            "SELECT profile, rate_w, variance FROM power_profile WHERE battery = ?1",
            (battery.to_string(),),
        )
        .await?;
    let mut states = HashMap::new();
    while let Some(row) = rows.next().await? {
        let Some(profile) = Profile::from_name(&text(&row, 0)?) else {
            continue;
        };
        let (Ok(rate_w), Ok(variance)) = (text(&row, 1)?.parse(), text(&row, 2)?.parse()) else {
            continue;
        };
        states.insert(profile, RateState { rate_w, variance });
    }
    Ok(states)
}

async fn try_save(battery: &str, profile: Profile, state: RateState) -> Result<(), BoxError> {
    let conn = open().await?;
    conn.execute(
        "INSERT INTO power_profile (battery, profile, rate_w, variance) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(battery, profile) DO UPDATE SET rate_w = ?3, variance = ?4",
        (
            battery.to_string(),
            profile.name().to_string(),
            state.rate_w.to_string(),
            state.variance.to_string(),
        ),
    )
    .await?;
    Ok(())
}
