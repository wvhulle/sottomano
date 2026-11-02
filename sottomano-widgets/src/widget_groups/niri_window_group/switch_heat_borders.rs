use iced::Task;
use sottomano_config::theme::palette::Palette;
use sottomano_sources::sqlite_state_store::{Columns, StoreError, flag, upsert_column, with_table};

use crate::{
    group_items::toggle_control::{IconToggle, SwitchPresses},
    widget_groups::niri_window_group::{
        NiriWindowMessage,
        niri_kdl_rules::{HeatedWindow, NiriRulesFile, NiriStyling, SwitchHeat},
    },
    widgets::phosphor::{Phosphor, Weight},
};

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS switch_heat_borders (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    shown INTEGER
)";

const SHOWN_BY_DEFAULT: bool = true;

#[derive(Debug, Default)]
pub(super) struct SwitchHeatBorders {
    shown: bool,
}

impl SwitchHeatBorders {
    pub(super) fn load() -> Task<NiriWindowMessage> {
        Task::perform(load_shown(), NiriWindowMessage::SwitchHeatLoaded)
    }

    pub(super) fn toggle(&self) -> IconToggle {
        IconToggle::new(
            "Switch heat",
            Phosphor::Fire(Weight::Regular),
            self.shown,
            SwitchPresses {
                turning_on: "tint each window's border warmer the more often you return to it, \
                             and ring the focused window",
                turning_off: "keep niri's own borders and focus ring",
            },
        )
    }

    pub(super) fn flip(&mut self) -> Task<NiriWindowMessage> {
        self.shown = !self.shown;
        Task::future(save_shown(self.shown)).discard()
    }

    pub(super) fn adopt_loaded(&mut self, shown: bool) {
        self.shown = shown;
    }

    pub(super) fn reheat(
        &self,
        rules_file: &mut NiriRulesFile,
        styling: NiriStyling,
        palette: &Palette,
        heated_windows: impl FnOnce() -> Vec<HeatedWindow>,
    ) -> Task<NiriWindowMessage> {
        let switch_heat = if self.shown {
            SwitchHeat::Shown {
                focus_ring_color: format!("{:x}", palette.primary.to_rgba8()),
                neutral_border_color: format!("{:x}", palette.background.to_rgba8()),
                windows: heated_windows(),
            }
        } else {
            SwitchHeat::Hidden
        };
        rules_file
            .reheat(styling, switch_heat)
            .map_or_else(Task::none, |written| Task::future(written).discard())
    }
}

async fn load_shown() -> bool {
    with_table(
        "switch heat borders: load failed",
        &[CREATE_TABLE],
        read_shown,
    )
    .await
    .unwrap_or(SHOWN_BY_DEFAULT)
}

async fn save_shown(shown: bool) -> bool {
    with_table(
        "switch heat borders: save failed",
        &[CREATE_TABLE],
        async |connection| {
            upsert_column(connection, "switch_heat_borders", "shown", flag(shown)).await
        },
    )
    .await
    .is_some()
}

async fn read_shown(connection: &turso::Connection) -> Result<bool, StoreError> {
    let mut rows = connection
        .query("SELECT shown FROM switch_heat_borders WHERE id = 0", ())
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(SHOWN_BY_DEFAULT);
    };
    Ok(columns
        .optional_flag(&row, "shown")
        .unwrap_or(SHOWN_BY_DEFAULT))
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::{flag, in_memory_connection, upsert_column};

    use super::{CREATE_TABLE, read_shown};

    #[tokio::test]
    async fn borders_show_until_turned_off() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        assert!(read_shown(&connection).await.unwrap());
        upsert_column(&connection, "switch_heat_borders", "shown", flag(false))
            .await
            .unwrap();
        assert!(!read_shown(&connection).await.unwrap());
    }
}
