use std::ops::Add;

use bytesize::ByteSize;
use chrono::{Datelike, Months, NaiveDate};
use sottomano_desktop_sources::sqlite_state_store::{Columns, StoreError, with_table};

pub const SCHEMA: [&str; 1] = ["CREATE TABLE IF NOT EXISTS metered_volume (
        day TEXT PRIMARY KEY,
        received_bytes INTEGER NOT NULL,
        sent_bytes INTEGER NOT NULL
    )"];

const DAY_FORMAT: &str = "%Y-%m-%d";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransferTotals {
    pub received: ByteSize,
    pub sent: ByteSize,
}

impl Add for TransferTotals {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            received: self.received + other.received,
            sent: self.sent + other.sent,
        }
    }
}

impl TransferTotals {
    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteredVolumeTotals {
    pub today: TransferTotals,
    pub cycle: TransferTotals,
    pub counting_since: NaiveDate,
}

impl MeteredVolumeTotals {
    #[must_use]
    pub fn starting(today: NaiveDate) -> Self {
        Self {
            today: TransferTotals::default(),
            cycle: TransferTotals::default(),
            counting_since: today,
        }
    }

    #[must_use]
    pub fn with_unsaved(self, unsaved: TransferTotals) -> Self {
        Self {
            today: self.today + unsaved,
            cycle: self.cycle + unsaved,
            ..self
        }
    }
}

#[must_use]
pub fn cycle_start(today: NaiveDate, cycle_start_day: u32) -> NaiveDate {
    let this_month_start = today.with_day(cycle_start_day).unwrap_or(today);
    if today.day() >= cycle_start_day {
        this_month_start
    } else {
        this_month_start
            .checked_sub_months(Months::new(1))
            .unwrap_or(this_month_start)
    }
}

pub async fn add_and_reload(
    today: NaiveDate,
    cycle_start_day: u32,
    transferred: TransferTotals,
) -> Option<MeteredVolumeTotals> {
    with_table("metered volume: save failed", &SCHEMA, async |connection| {
        add_transferred(connection, today, transferred).await?;
        read_totals(connection, today, cycle_start(today, cycle_start_day)).await
    })
    .await
}

pub async fn load(today: NaiveDate, cycle_start_day: u32) -> Option<MeteredVolumeTotals> {
    with_table("metered volume: load failed", &SCHEMA, async |connection| {
        read_totals(connection, today, cycle_start(today, cycle_start_day)).await
    })
    .await
}

fn stored_bytes(size: ByteSize) -> i64 {
    i64::try_from(size.as_u64()).unwrap_or(i64::MAX)
}

fn loaded_bytes(stored: Option<i64>) -> ByteSize {
    ByteSize::b(
        stored
            .and_then(|bytes| u64::try_from(bytes).ok())
            .unwrap_or_default(),
    )
}

async fn add_transferred(
    connection: &turso::Connection,
    day: NaiveDate,
    transferred: TransferTotals,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO metered_volume (day, received_bytes, sent_bytes) VALUES (?1, ?2, ?3)
             ON CONFLICT(day) DO UPDATE SET
                 received_bytes = received_bytes + ?2,
                 sent_bytes = sent_bytes + ?3",
            (
                day.format(DAY_FORMAT).to_string(),
                stored_bytes(transferred.received),
                stored_bytes(transferred.sent),
            ),
        )
        .await?;
    Ok(())
}

async fn summed_since(
    connection: &turso::Connection,
    first_day: NaiveDate,
    last_day: NaiveDate,
) -> Result<TransferTotals, StoreError> {
    let mut summed_rows = connection
        .query(
            "SELECT SUM(received_bytes) AS received, SUM(sent_bytes) AS sent
             FROM metered_volume WHERE day >= ?1 AND day <= ?2",
            (
                first_day.format(DAY_FORMAT).to_string(),
                last_day.format(DAY_FORMAT).to_string(),
            ),
        )
        .await?;
    let columns = Columns::of(&summed_rows);
    let Some(summed_row) = summed_rows.next().await? else {
        return Ok(TransferTotals::default());
    };
    Ok(TransferTotals {
        received: loaded_bytes(columns.optional_integer(&summed_row, "received")),
        sent: loaded_bytes(columns.optional_integer(&summed_row, "sent")),
    })
}

async fn first_counted_day(
    connection: &turso::Connection,
) -> Result<Option<NaiveDate>, StoreError> {
    let mut first_day_rows = connection
        .query("SELECT MIN(day) AS first_day FROM metered_volume", ())
        .await?;
    let columns = Columns::of(&first_day_rows);
    Ok(first_day_rows
        .next()
        .await?
        .and_then(|first_day_row| columns.optional_text(&first_day_row, "first_day"))
        .and_then(|first_day| NaiveDate::parse_from_str(&first_day, DAY_FORMAT).ok()))
}

async fn read_totals(
    connection: &turso::Connection,
    today: NaiveDate,
    cycle_first_day: NaiveDate,
) -> Result<MeteredVolumeTotals, StoreError> {
    Ok(MeteredVolumeTotals {
        today: summed_since(connection, today, today).await?,
        cycle: summed_since(connection, cycle_first_day, today).await?,
        counting_since: first_counted_day(connection).await?.unwrap_or(today),
    })
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::sqlite_state_store::in_memory_connection;

    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn totals(received: u64, sent: u64) -> TransferTotals {
        TransferTotals {
            received: ByteSize::b(received),
            sent: ByteSize::b(sent),
        }
    }

    #[test]
    fn cycle_starts_this_month_once_its_day_has_come() {
        assert_eq!(cycle_start(date(2026, 10, 20), 15), date(2026, 10, 15));
        assert_eq!(cycle_start(date(2026, 10, 15), 15), date(2026, 10, 15));
    }

    #[test]
    fn cycle_starts_last_month_before_its_day() {
        assert_eq!(cycle_start(date(2026, 10, 3), 15), date(2026, 9, 15));
        assert_eq!(cycle_start(date(2026, 1, 3), 15), date(2025, 12, 15));
    }

    #[tokio::test]
    async fn additions_on_one_day_accumulate() {
        let connection = in_memory_connection(&SCHEMA).await;
        let today = date(2026, 10, 28);
        add_transferred(&connection, today, totals(100, 10))
            .await
            .unwrap();
        add_transferred(&connection, today, totals(50, 5))
            .await
            .unwrap();
        let read_back = read_totals(&connection, today, date(2026, 10, 1))
            .await
            .unwrap();
        assert_eq!(read_back.today, totals(150, 15));
        assert_eq!(read_back.counting_since, today);
    }

    #[tokio::test]
    async fn cycle_sums_only_days_since_its_start() {
        let connection = in_memory_connection(&SCHEMA).await;
        add_transferred(&connection, date(2026, 9, 30), totals(1_000, 100))
            .await
            .unwrap();
        add_transferred(&connection, date(2026, 10, 2), totals(200, 20))
            .await
            .unwrap();
        add_transferred(&connection, date(2026, 10, 28), totals(30, 3))
            .await
            .unwrap();
        let read_back = read_totals(&connection, date(2026, 10, 28), date(2026, 10, 1))
            .await
            .unwrap();
        assert_eq!(read_back.today, totals(30, 3));
        assert_eq!(read_back.cycle, totals(230, 23));
        assert_eq!(read_back.counting_since, date(2026, 9, 30));
    }

    #[tokio::test]
    async fn empty_store_counts_from_today() {
        let connection = in_memory_connection(&SCHEMA).await;
        let today = date(2026, 10, 28);
        let read_back = read_totals(&connection, today, date(2026, 10, 1))
            .await
            .unwrap();
        assert_eq!(read_back, MeteredVolumeTotals::starting(today));
    }
}
