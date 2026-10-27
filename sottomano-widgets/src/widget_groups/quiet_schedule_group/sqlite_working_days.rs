use chrono::{Weekday, WeekdaySet};
use sottomano_sources::sqlite_state_store::{Columns, StoreError, with_table};

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS working_days (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    weekday_bits INTEGER NOT NULL
)";

pub const MONDAY_TO_FRIDAY: WeekdaySet = WeekdaySet::from_array([
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
]);

#[must_use]
pub fn with_day_toggled(working_days: WeekdaySet, day: Weekday) -> WeekdaySet {
    let mut toggled_days = working_days;
    if !toggled_days.remove(day) {
        toggled_days.insert(day);
    }
    toggled_days
}

fn weekday_bits(working_days: WeekdaySet) -> i64 {
    working_days
        .iter(Weekday::Mon)
        .map(|day| 1 << day.num_days_from_monday())
        .sum()
}

fn weekdays_of_bits(bits: i64) -> WeekdaySet {
    WeekdaySet::ALL
        .iter(Weekday::Mon)
        .filter(|day| bits & (1 << day.num_days_from_monday()) != 0)
        .collect()
}

pub async fn load() -> WeekdaySet {
    with_table("working days: load failed", &[CREATE_TABLE], read_days)
        .await
        .unwrap_or(MONDAY_TO_FRIDAY)
}

pub async fn save(working_days: WeekdaySet) -> bool {
    with_table(
        "working days: save failed",
        &[CREATE_TABLE],
        async |connection| write_days(connection, working_days).await,
    )
    .await
    .is_some()
}

async fn read_days(connection: &turso::Connection) -> Result<WeekdaySet, StoreError> {
    let mut rows = connection
        .query("SELECT weekday_bits FROM working_days WHERE id = 0", ())
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(MONDAY_TO_FRIDAY);
    };
    Ok(weekdays_of_bits(columns.integer(&row, "weekday_bits")?))
}

async fn write_days(
    connection: &turso::Connection,
    working_days: WeekdaySet,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO working_days (id, weekday_bits) VALUES (0, ?1)
         ON CONFLICT(id) DO UPDATE SET weekday_bits = ?1",
            (weekday_bits(working_days),),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Weekday, WeekdaySet};
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{CREATE_TABLE, MONDAY_TO_FRIDAY, read_days, with_day_toggled, write_days};

    #[tokio::test]
    async fn an_unsaved_week_reads_as_monday_to_friday() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;

        assert_eq!(read_days(&connection).await.unwrap(), MONDAY_TO_FRIDAY);
    }

    #[tokio::test]
    async fn chosen_days_survive_a_round_trip() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        let chosen_days = WeekdaySet::from_array([Weekday::Mon, Weekday::Tue, Weekday::Thu]);
        write_days(&connection, chosen_days).await.unwrap();

        assert_eq!(read_days(&connection).await.unwrap(), chosen_days);
    }

    #[tokio::test]
    async fn an_empty_week_round_trips() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        write_days(&connection, WeekdaySet::EMPTY).await.unwrap();

        assert_eq!(read_days(&connection).await.unwrap(), WeekdaySet::EMPTY);
    }

    #[test]
    fn toggling_a_day_twice_restores_the_week() {
        assert!(!with_day_toggled(MONDAY_TO_FRIDAY, Weekday::Wed).contains(Weekday::Wed));
        assert!(with_day_toggled(MONDAY_TO_FRIDAY, Weekday::Sat).contains(Weekday::Sat));
        assert_eq!(
            with_day_toggled(
                with_day_toggled(MONDAY_TO_FRIDAY, Weekday::Wed),
                Weekday::Wed
            ),
            MONDAY_TO_FRIDAY
        );
    }
}
