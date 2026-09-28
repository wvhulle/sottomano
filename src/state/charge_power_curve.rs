use crate::{
    data::battery_forecast::{CHARGE_BINS, ChargeCurve, PowerBin, ResidualProcess},
    state::{Columns, StoreError, open},
};

pub(crate) const CREATE_TABLES: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS battery_charge_curve (
        battery TEXT NOT NULL,
        bin INTEGER NOT NULL,
        mean_w REAL NOT NULL,
        variance REAL NOT NULL,
        PRIMARY KEY (battery, bin)
    )",
    "CREATE TABLE IF NOT EXISTS battery_charge_residual (
        battery TEXT PRIMARY KEY NOT NULL,
        variance REAL NOT NULL,
        lag_covariance REAL NOT NULL
    )",
];

pub async fn load(battery: String) -> Box<ChargeCurve> {
    Box::new(try_load(&battery).await.unwrap_or_else(|e| {
        tracing::warn!("charge curve: load failed: {e}");
        ChargeCurve::default()
    }))
}

pub async fn save(battery: String, bin: usize, curve: ChargeCurve) {
    if let Err(e) = try_save(&battery, bin, &curve).await {
        tracing::warn!("charge curve: save failed: {e}");
    }
}

async fn try_load(battery: &str) -> Result<ChargeCurve, StoreError> {
    let conn = open().await?;
    read_curve(&conn, battery).await
}

async fn try_save(battery: &str, bin: usize, curve: &ChargeCurve) -> Result<(), StoreError> {
    let conn = open().await?;
    write_bin(&conn, battery, bin, curve).await
}

async fn read_curve(conn: &turso::Connection, battery: &str) -> Result<ChargeCurve, StoreError> {
    let mut curve = ChargeCurve::default();
    let mut bin_rows = conn
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
    let mut residual_rows = conn
        .query(
            "SELECT variance, lag_covariance FROM battery_charge_residual WHERE battery = ?1",
            (battery,),
        )
        .await?;
    let columns = Columns::of(&residual_rows);
    if let Some(row) = residual_rows.next().await? {
        curve.residuals = ResidualProcess {
            variance: columns.real(&row, "variance")?,
            lag_covariance: columns.real(&row, "lag_covariance")?,
            previous: None,
        };
    }
    Ok(curve)
}

async fn write_bin(
    conn: &turso::Connection,
    battery: &str,
    bin: usize,
    curve: &ChargeCurve,
) -> Result<(), StoreError> {
    if let Some(power) = curve.bins.get(bin).copied().flatten() {
        conn.execute(
            "INSERT INTO battery_charge_curve (battery, bin, mean_w, variance)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(battery, bin) DO UPDATE SET mean_w = ?3, variance = ?4",
            (battery, i64::from(bin as u32), power.mean_w, power.variance),
        )
        .await?;
    }
    conn.execute(
        "INSERT INTO battery_charge_residual (battery, variance, lag_covariance)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(battery) DO UPDATE SET variance = ?2, lag_covariance = ?3",
        (
            battery,
            curve.residuals.variance,
            curve.residuals.lag_covariance,
        ),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        for statement in CREATE_TABLES {
            conn.execute(statement, ()).await.unwrap();
        }
        conn
    }

    #[tokio::test]
    async fn bins_and_residual_survive_a_round_trip() {
        let conn = connection().await;
        let mut curve = ChargeCurve::default();
        curve.bins[3] = Some(PowerBin {
            mean_w: 28.5,
            variance: 0.7,
        });
        curve.residuals = ResidualProcess {
            variance: 2.5,
            lag_covariance: 1.5,
            previous: None,
        };
        write_bin(&conn, "BAT0", 3, &curve).await.unwrap();

        assert_eq!(read_curve(&conn, "BAT0").await.unwrap(), curve);
    }

    #[tokio::test]
    async fn unknown_battery_has_an_empty_curve() {
        let conn = connection().await;
        assert_eq!(
            read_curve(&conn, "BAT9").await.unwrap(),
            ChargeCurve::default()
        );
    }
}
