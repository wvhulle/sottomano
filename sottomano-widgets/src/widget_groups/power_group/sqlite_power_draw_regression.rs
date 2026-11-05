use std::collections::HashMap;

use nalgebra::{Matrix3, Vector3};
use sottomano_desktop_sources::{
    battery_power_profiles::{
        battery_time_forecast::RegressionState, power_profiles_daemon::Profile,
    },
    sqlite_state_store::{Columns, StoreError, with_table},
};

pub const SCHEMA: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS battery_draw_model (
        battery TEXT NOT NULL,
        profile TEXT NOT NULL,
        base_w REAL NOT NULL,
        k_bright REAL NOT NULL,
        k_load REAL NOT NULL,
        PRIMARY KEY (battery, profile)
    )",
    "CREATE TABLE IF NOT EXISTS battery_draw_covariance (
        battery TEXT NOT NULL,
        profile TEXT NOT NULL,
        row_index INTEGER NOT NULL,
        column_index INTEGER NOT NULL,
        covariance REAL NOT NULL,
        PRIMARY KEY (battery, profile, row_index, column_index),
        CHECK (row_index <= column_index AND column_index < 3)
    )",
];

const UPPER_TRIANGLE_ENTRIES: usize = 6;

pub async fn load(battery: String) -> HashMap<Profile, RegressionState> {
    with_table(
        "power regression: load failed",
        &SCHEMA,
        async |connection| read_models(connection, &battery).await,
    )
    .await
    .unwrap_or_default()
}

pub async fn save(battery: String, profile: Profile, state: RegressionState) {
    with_table(
        "power regression: save failed",
        &SCHEMA,
        async |connection| write_model(connection, &battery, profile, state).await,
    )
    .await;
}

async fn read_covariances(
    connection: &turso::Connection,
    battery: &str,
) -> Result<HashMap<String, Vec<(usize, usize, f64)>>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT profile, row_index, column_index, covariance
             FROM battery_draw_covariance WHERE battery = ?1",
            (battery,),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut entries: HashMap<String, Vec<(usize, usize, f64)>> = HashMap::new();
    while let Some(row) = rows.next().await? {
        let index_of = |name: &str| -> Result<usize, StoreError> {
            Ok(usize::try_from(columns.integer(&row, name)?).unwrap_or_default())
        };
        entries
            .entry(columns.text(&row, "profile")?)
            .or_default()
            .push((
                index_of("row_index")?,
                index_of("column_index")?,
                columns.real(&row, "covariance")?,
            ));
    }
    Ok(entries)
}

fn symmetric_covariance(upper_triangle: &[(usize, usize, f64)]) -> Option<Matrix3<f64>> {
    (upper_triangle.len() == UPPER_TRIANGLE_ENTRIES).then(|| {
        upper_triangle.iter().fold(
            Matrix3::zeros(),
            |mut covariance, &(row_index, column_index, value)| {
                covariance[(row_index, column_index)] = value;
                covariance[(column_index, row_index)] = value;
                covariance
            },
        )
    })
}

async fn read_models(
    connection: &turso::Connection,
    battery: &str,
) -> Result<HashMap<Profile, RegressionState>, StoreError> {
    let covariances = read_covariances(connection, battery).await?;
    let mut rows = connection
        .query(
            "SELECT profile, base_w, k_bright, k_load FROM battery_draw_model WHERE battery = ?1",
            (battery,),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut states = HashMap::new();
    while let Some(row) = rows.next().await? {
        let profile_name = columns.text(&row, "profile")?;
        let Some(profile) = Profile::from_name(&profile_name) else {
            continue;
        };
        let Some(covariance) = covariances
            .get(&profile_name)
            .and_then(|upper_triangle| symmetric_covariance(upper_triangle))
        else {
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
                covariance,
            },
        );
    }
    Ok(states)
}

async fn write_model(
    connection: &turso::Connection,
    battery: &str,
    profile: Profile,
    state: RegressionState,
) -> Result<(), StoreError> {
    let profile_name = profile.name().to_string();
    connection
        .execute(
            "INSERT INTO battery_draw_model (battery, profile, base_w, k_bright, k_load)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(battery, profile)
         DO UPDATE SET base_w = ?3, k_bright = ?4, k_load = ?5",
            (
                battery,
                profile_name.clone(),
                state.coefficients[0],
                state.coefficients[1],
                state.coefficients[2],
            ),
        )
        .await?;
    for row_index in 0..3_usize {
        for column_index in row_index..3_usize {
            connection
                .execute(
                    "INSERT INTO battery_draw_covariance
                         (battery, profile, row_index, column_index, covariance)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(battery, profile, row_index, column_index)
                     DO UPDATE SET covariance = ?5",
                    (
                        battery,
                        profile_name.clone(),
                        i64::try_from(row_index).unwrap_or_default(),
                        i64::try_from(column_index).unwrap_or_default(),
                        state.covariance[(row_index, column_index)],
                    ),
                )
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use nalgebra::{Matrix3, Vector3};
    use sottomano_desktop_sources::sqlite_state_store::in_memory_connection;

    use super::{Profile, RegressionState, SCHEMA, read_models, write_model};

    fn distinct_state() -> RegressionState {
        RegressionState {
            coefficients: Vector3::new(1.5, 2.5, 3.5),
            covariance: Matrix3::new(11.0, 12.0, 13.0, 12.0, 22.0, 23.0, 13.0, 23.0, 33.0),
        }
    }

    #[tokio::test]
    async fn coefficients_and_covariance_survive_a_round_trip() {
        let connection = in_memory_connection(&SCHEMA).await;
        write_model(&connection, "BAT0", Profile::Balanced, distinct_state())
            .await
            .unwrap();

        let models = read_models(&connection, "BAT0").await.unwrap();
        assert_eq!(models[&Profile::Balanced], distinct_state());
    }
}
