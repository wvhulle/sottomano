use std::collections::HashMap;

use nalgebra::{Matrix3, Vector3};

use crate::{
    data::source::power_profile::{Profile, RegressionState},
    state::{BoxError, open, text},
};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS power_regression (
    battery TEXT NOT NULL,
    profile TEXT NOT NULL,
    base_w TEXT NOT NULL,
    k_bright TEXT NOT NULL,
    k_load TEXT NOT NULL,
    covariance TEXT NOT NULL,
    PRIMARY KEY (battery, profile)
)";

pub(crate) const DROP_LEGACY_TABLE: &str = "DROP TABLE IF EXISTS power_profile";

pub async fn load(battery: String) -> HashMap<Profile, RegressionState> {
    try_load(&battery).await.unwrap_or_else(|e| {
        eprintln!("power regression: load failed: {e}");
        HashMap::new()
    })
}

pub async fn save(battery: String, profile: Profile, state: RegressionState) {
    if let Err(e) = try_save(&battery, profile, state).await {
        eprintln!("power regression: save failed: {e}");
    }
}

async fn try_load(battery: &str) -> Result<HashMap<Profile, RegressionState>, BoxError> {
    let conn = open().await?;
    let mut rows = conn
        .query(
            "SELECT profile, base_w, k_bright, k_load, covariance FROM power_regression
             WHERE battery = ?1",
            (battery.to_string(),),
        )
        .await?;
    let mut states = HashMap::new();
    while let Some(row) = rows.next().await? {
        let Some(profile) = Profile::from_name(&text(&row, 0)?) else {
            continue;
        };
        let (Ok(base_w), Ok(k_bright), Ok(k_load)) = (
            text(&row, 1)?.parse(),
            text(&row, 2)?.parse(),
            text(&row, 3)?.parse(),
        ) else {
            continue;
        };
        let Some(covariance) = parse_covariance(&text(&row, 4)?) else {
            continue;
        };
        states.insert(
            profile,
            RegressionState {
                coefficients: Vector3::new(base_w, k_bright, k_load),
                covariance,
            },
        );
    }
    Ok(states)
}

async fn try_save(battery: &str, profile: Profile, state: RegressionState) -> Result<(), BoxError> {
    let conn = open().await?;
    conn.execute(
        "INSERT INTO power_regression (battery, profile, base_w, k_bright, k_load, covariance)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(battery, profile)
         DO UPDATE SET base_w = ?3, k_bright = ?4, k_load = ?5, covariance = ?6",
        (
            battery.to_string(),
            profile.name().to_string(),
            state.coefficients[0].to_string(),
            state.coefficients[1].to_string(),
            state.coefficients[2].to_string(),
            format_covariance(&state.covariance),
        ),
    )
    .await?;
    Ok(())
}

fn format_covariance(covariance: &Matrix3<f64>) -> String {
    covariance
        .as_slice()
        .iter()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_covariance(text: &str) -> Option<Matrix3<f64>> {
    let values: Vec<f64> = text
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    (values.len() == 9).then(|| Matrix3::from_column_slice(&values))
}
