use sottomano_sources::{
    battery_power_profiles::battery_time_forecast::{CHARGE_BINS, ChargeCurve, PowerBin},
    sqlite_state_store::{Columns, StoreError, with_table},
};

pub const SCHEMA: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS battery_charge_curve (
        battery TEXT NOT NULL,
        bin INTEGER NOT NULL,
        mean_w REAL NOT NULL,
        variance REAL NOT NULL,
        PRIMARY KEY (battery, bin)
    )",
    "DROP TABLE IF EXISTS battery_charge_residual",
];

pub async fn load(battery: String) -> Box<ChargeCurve> {
    Box::new(
        with_table("charge curve: load failed", &SCHEMA, async |connection| {
            read_curve(connection, &battery).await
        })
        .await
        .unwrap_or_default(),
    )
}

pub async fn save(battery: String, bin: usize, curve: ChargeCurve) {
    with_table("charge curve: save failed", &SCHEMA, async |connection| {
        write_bin(connection, &battery, bin, &curve).await
    })
    .await;
}

async fn read_curve(
    connection: &turso::Connection,
    battery: &str,
) -> Result<ChargeCurve, StoreError> {
    let mut curve = ChargeCurve::default();
    let mut bin_rows = connection
        .query(
            "SELECT bin, mean_w, variance FROM battery_charge_curve WHERE battery = ?1",
            (battery,),
        )
        .await?;
    let columns = Columns::of(&bin_rows);
    while let Some(row) = bin_rows.next().await? {
        let Some(bin) = usize::try_from(columns.integer(&row, "bin")?)
            .ok()
            .filter(|bin| *bin < CHARGE_BINS)
        else {
            continue;
        };
        curve.bins[bin] = Some(PowerBin {
            mean_w: columns.real(&row, "mean_w")?,
            variance: columns.real(&row, "variance")?,
        });
    }
    Ok(curve)
}

async fn write_bin(
    connection: &turso::Connection,
    battery: &str,
    bin: usize,
    curve: &ChargeCurve,
) -> Result<(), StoreError> {
    if let Some(power) = curve.bins.get(bin).copied().flatten() {
        connection
            .execute(
                "INSERT INTO battery_charge_curve (battery, bin, mean_w, variance)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(battery, bin) DO UPDATE SET mean_w = ?3, variance = ?4",
                (battery, i64::from(bin as u32), power.mean_w, power.variance),
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::*;

    #[tokio::test]
    async fn bins_survive_a_round_trip() {
        let connection = in_memory_connection(&SCHEMA).await;
        let mut curve = ChargeCurve::default();
        curve.bins[3] = Some(PowerBin {
            mean_w: 28.5,
            variance: 0.7,
        });
        write_bin(&connection, "BAT0", 3, &curve).await.unwrap();

        assert_eq!(read_curve(&connection, "BAT0").await.unwrap(), curve);
    }

    #[tokio::test]
    async fn unknown_battery_has_an_empty_curve() {
        let connection = in_memory_connection(&SCHEMA).await;
        assert_eq!(
            read_curve(&connection, "BAT9").await.unwrap(),
            ChargeCurve::default()
        );
    }
}
