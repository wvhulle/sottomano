use std::collections::HashMap;

use nalgebra::{Matrix3, Vector3};

use crate::{
    data::{battery_forecast::RegressionState, source::power_profile::Profile},
    state::{Columns, StoreError, open},
};

const RETIRED_COLUMNS: [&str; 2] = ["residual_variance", "residual_lag_covariance"];

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS battery_draw_model (
    battery TEXT NOT NULL,
    profile TEXT NOT NULL,
    base_w REAL NOT NULL,
    k_bright REAL NOT NULL,
    k_load REAL NOT NULL,
    cov_00 REAL NOT NULL,
    cov_01 REAL NOT NULL,
    cov_02 REAL NOT NULL,
    cov_10 REAL NOT NULL,
    cov_11 REAL NOT NULL,
    cov_12 REAL NOT NULL,
    cov_20 REAL NOT NULL,
    cov_21 REAL NOT NULL,
    cov_22 REAL NOT NULL,
    PRIMARY KEY (battery, profile)
)";

pub(crate) async fn drop_residual_columns(conn: &turso::Connection) -> Result<(), StoreError> {
    let mut rows = conn
        .query("PRAGMA table_info(battery_draw_model)", ())
        .await?;
    let columns = Columns::of(&rows);
    let mut present = Vec::new();
    while let Some(row) = rows.next().await? {
        present.push(columns.text(&row, "name")?);
    }
    for retired in RETIRED_COLUMNS
        .iter()
        .filter(|retired| present.iter().any(|name| name == *retired))
    {
        conn.execute(
            &format!("ALTER TABLE battery_draw_model DROP COLUMN {retired}"),
            (),
        )
        .await?;
    }
    Ok(())
}

pub async fn load(battery: String) -> HashMap<Profile, RegressionState> {
    try_load(&battery).await.unwrap_or_else(|e| {
        tracing::warn!("power regression: load failed: {e}");
        HashMap::new()
    })
}

pub async fn save(battery: String, profile: Profile, state: RegressionState) {
    if let Err(e) = try_save(&battery, profile, state).await {
        tracing::warn!("power regression: save failed: {e}");
    }
}

async fn try_load(battery: &str) -> Result<HashMap<Profile, RegressionState>, StoreError> {
    let conn = open().await?;
    read_models(&conn, battery).await
}

async fn read_models(
    conn: &turso::Connection,
    battery: &str,
) -> Result<HashMap<Profile, RegressionState>, StoreError> {
    let mut rows = conn
        .query(
            "SELECT profile, base_w, k_bright, k_load,
                    cov_00, cov_01, cov_02, cov_10, cov_11, cov_12, cov_20, cov_21, cov_22
             FROM battery_draw_model WHERE battery = ?1",
            (battery,),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut states = HashMap::new();
    while let Some(row) = rows.next().await? {
        let Some(profile) = Profile::from_name(&columns.text(&row, "profile")?) else {
            continue;
        };
        states.insert(
            profile,
            RegressionState {
                coefficients: Vector3::new(
                    columns.real(&row, "base_w")?,
                    columns.real(&row, "k_bright")?,
                    columns.real(&row, "k_load")?,
                ),
                covariance: Matrix3::new(
                    columns.real(&row, "cov_00")?,
                    columns.real(&row, "cov_01")?,
                    columns.real(&row, "cov_02")?,
                    columns.real(&row, "cov_10")?,
                    columns.real(&row, "cov_11")?,
                    columns.real(&row, "cov_12")?,
                    columns.real(&row, "cov_20")?,
                    columns.real(&row, "cov_21")?,
                    columns.real(&row, "cov_22")?,
                ),
            },
        );
    }
    Ok(states)
}

async fn try_save(
    battery: &str,
    profile: Profile,
    state: RegressionState,
) -> Result<(), StoreError> {
    let conn = open().await?;
    write_model(&conn, battery, profile, state).await
}

async fn write_model(
    conn: &turso::Connection,
    battery: &str,
    profile: Profile,
    state: RegressionState,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO battery_draw_model (battery, profile, base_w, k_bright, k_load,
             cov_00, cov_01, cov_02, cov_10, cov_11, cov_12, cov_20, cov_21, cov_22)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(battery, profile)
         DO UPDATE SET base_w = ?3, k_bright = ?4, k_load = ?5,
             cov_00 = ?6, cov_01 = ?7, cov_02 = ?8,
             cov_10 = ?9, cov_11 = ?10, cov_12 = ?11,
             cov_20 = ?12, cov_21 = ?13, cov_22 = ?14",
        (
            battery,
            profile.name().to_string(),
            state.coefficients[0],
            state.coefficients[1],
            state.coefficients[2],
            state.covariance[(0, 0)],
            state.covariance[(0, 1)],
            state.covariance[(0, 2)],
            state.covariance[(1, 0)],
            state.covariance[(1, 1)],
            state.covariance[(1, 2)],
            state.covariance[(2, 0)],
            state.covariance[(2, 1)],
            state.covariance[(2, 2)],
        ),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use nalgebra::{Matrix3, Vector3};

    use super::{
        CREATE_TABLE, Profile, RegressionState, drop_residual_columns, read_models, write_model,
    };

    async fn empty_connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        database.connect().unwrap()
    }

    async fn connection() -> turso::Connection {
        let conn = empty_connection().await;
        conn.execute(CREATE_TABLE, ()).await.unwrap();
        conn
    }

    fn distinct_state() -> RegressionState {
        RegressionState {
            coefficients: Vector3::new(1.5, 2.5, 3.5),
            covariance: Matrix3::new(11.0, 12.0, 13.0, 21.0, 22.0, 23.0, 31.0, 32.0, 33.0),
        }
    }

    #[tokio::test]
    async fn residual_columns_are_dropped_once_and_rows_survive() {
        let conn = empty_connection().await;
        let with_residuals = CREATE_TABLE.replace(
            "cov_22 REAL NOT NULL,",
            "cov_22 REAL NOT NULL,
             residual_variance REAL NOT NULL,
             residual_lag_covariance REAL NOT NULL,",
        );
        conn.execute(&with_residuals, ()).await.unwrap();
        conn.execute(
            "INSERT INTO battery_draw_model VALUES
             ('BAT0', 'balanced', 1.5, 2.5, 3.5, 11, 12, 13, 21, 22, 23, 31, 32, 33, 4.5, 5.5)",
            (),
        )
        .await
        .unwrap();

        drop_residual_columns(&conn).await.unwrap();
        drop_residual_columns(&conn).await.unwrap();

        let models = read_models(&conn, "BAT0").await.unwrap();
        assert_eq!(models[&Profile::Balanced], distinct_state());
        write_model(&conn, "BAT0", Profile::Balanced, distinct_state())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn coefficients_and_covariance_survive_a_round_trip() {
        let conn = connection().await;
        write_model(&conn, "BAT0", Profile::Balanced, distinct_state())
            .await
            .unwrap();

        let models = read_models(&conn, "BAT0").await.unwrap();
        assert_eq!(models[&Profile::Balanced], distinct_state());
    }
}
