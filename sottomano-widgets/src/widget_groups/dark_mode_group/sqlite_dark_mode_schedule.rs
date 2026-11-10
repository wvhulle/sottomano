use chrono::TimeDelta;
use sottomano_desktop_sources::{
    screen_brightness_color::solar_switch_schedule::SunEventOffsets,
    sqlite_state_store::{Columns, StoreError, flag, in_state_store, with_state_store},
};

use crate::group_items::{
    reading_display_state::ReadingDisplayState, toggle_control::SwitchPosition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSwitchArming {
    Armed,
    Disarmed,
}

impl AutoSwitchArming {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            AutoSwitchArming::Armed => AutoSwitchArming::Disarmed,
            AutoSwitchArming::Disarmed => AutoSwitchArming::Armed,
        }
    }
}

impl From<AutoSwitchArming> for SwitchPosition {
    fn from(auto_switch_arming: AutoSwitchArming) -> Self {
        match auto_switch_arming {
            AutoSwitchArming::Armed => SwitchPosition::On,
            AutoSwitchArming::Disarmed => SwitchPosition::Off,
        }
    }
}

impl From<AutoSwitchArming> for ReadingDisplayState {
    fn from(auto_switch_arming: AutoSwitchArming) -> Self {
        match auto_switch_arming {
            AutoSwitchArming::Armed => ReadingDisplayState::On,
            AutoSwitchArming::Disarmed => ReadingDisplayState::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DarkModeSchedule {
    pub auto_switch_arming: AutoSwitchArming,
    pub sun_event_offsets: SunEventOffsets,
}

impl Default for DarkModeSchedule {
    fn default() -> Self {
        Self {
            auto_switch_arming: AutoSwitchArming::Armed,
            sun_event_offsets: SunEventOffsets::default(),
        }
    }
}

pub async fn load() -> DarkModeSchedule {
    with_state_store("dark mode schedule: load failed", read_schedule)
        .await
        .unwrap_or_default()
}

pub async fn save(schedule: DarkModeSchedule) -> Result<(), StoreError> {
    in_state_store(async |connection| write_schedule(connection, schedule).await).await
}

async fn read_schedule(connection: &turso::Connection) -> Result<DarkModeSchedule, StoreError> {
    let mut rows = connection
        .query(
            "SELECT armed, sunrise_offset, sunset_offset FROM dark_mode_schedule WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(DarkModeSchedule::default());
    };
    Ok(DarkModeSchedule {
        auto_switch_arming: match columns.integer(&row, "armed")? {
            0 => AutoSwitchArming::Disarmed,
            _ => AutoSwitchArming::Armed,
        },
        sun_event_offsets: SunEventOffsets {
            sunrise_offset: TimeDelta::seconds(columns.integer(&row, "sunrise_offset")?),
            sunset_offset: TimeDelta::seconds(columns.integer(&row, "sunset_offset")?),
        },
    })
}

async fn write_schedule(
    connection: &turso::Connection,
    schedule: DarkModeSchedule,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO dark_mode_schedule (id, armed, sunrise_offset, sunset_offset)
         VALUES (0, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET armed = ?1, sunrise_offset = ?2, sunset_offset = ?3",
            (
                flag(schedule.auto_switch_arming == AutoSwitchArming::Armed),
                schedule.sun_event_offsets.sunrise_offset.num_seconds(),
                schedule.sun_event_offsets.sunset_offset.num_seconds(),
            ),
        )
        .await?;
    Ok(())
}
