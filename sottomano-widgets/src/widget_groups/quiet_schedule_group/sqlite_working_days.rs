use chrono::{Weekday, WeekdaySet};
use sottomano_sources::sqlite_state_store::{Columns, StoreError, with_table};

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS working_days (
    days_from_monday INTEGER PRIMARY KEY CHECK (days_from_monday BETWEEN 0 AND 6),
    is_working INTEGER NOT NULL
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
        .query("SELECT days_from_monday, is_working FROM working_days", ())
        .await?;
    let columns = Columns::of(&rows);
    let mut stored_any_day = false;
    let mut working_days = WeekdaySet::EMPTY;
    while let Some(row) = rows.next().await? {
        stored_any_day = true;
        let weekday = WeekdaySet::ALL
            .iter(Weekday::Mon)
            .nth(usize::try_from(columns.integer(&row, "days_from_monday")?).unwrap_or(7));
        if let Some(day) = weekday.filter(|_| {
            columns
                .integer(&row, "is_working")
                .is_ok_and(|is_working| is_working != 0)
        }) {
            working_days.insert(day);
        }
    }
    Ok(if stored_any_day {
        working_days
    } else {
        MONDAY_TO_FRIDAY
    })
}

async fn write_days(
    connection: &turso::Connection,
    working_days: WeekdaySet,
) -> Result<(), StoreError> {
    for day in WeekdaySet::ALL.iter(Weekday::Mon) {
        connection
            .execute(
                "INSERT INTO working_days (days_from_monday, is_working) VALUES (?1, ?2)
                 ON CONFLICT(days_from_monday) DO UPDATE SET is_working = ?2",
                (
                    i64::from(day.num_days_from_monday()),
                    i64::from(working_days.contains(day)),
                ),
            )
            .await?;
    }
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
