use iced::Task;
use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, flag, in_table, upsert_column, with_table,
};
use sottomano_toml_config::theme::palette::Palette;

use crate::{
    background_work::logged_failure_task,
    group_items::toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    widget_groups::niri_window_group::{
        NiriWindowMessage,
        niri_kdl_rules::{
            HeatedWindow, NiriRulesFile, NiriStyling, RulesSaveError, SaveStep, SwitchHeat,
        },
    },
    widgets::phosphor::{Phosphor, Weight},
};

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS switch_heat_borders (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    shown INTEGER NOT NULL
)";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SwitchHeatVisibility {
    #[default]
    Shown,
    Hidden,
}

impl SwitchHeatVisibility {
    const fn toggled(self) -> Self {
        match self {
            SwitchHeatVisibility::Shown => SwitchHeatVisibility::Hidden,
            SwitchHeatVisibility::Hidden => SwitchHeatVisibility::Shown,
        }
    }
}

impl From<SwitchHeatVisibility> for SwitchPosition {
    fn from(heat_visibility: SwitchHeatVisibility) -> Self {
        match heat_visibility {
            SwitchHeatVisibility::Shown => SwitchPosition::On,
            SwitchHeatVisibility::Hidden => SwitchPosition::Off,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct SwitchHeatBorders {
    heat_visibility: SwitchHeatVisibility,
}

impl SwitchHeatBorders {
    pub(super) fn load() -> Task<NiriWindowMessage> {
        Task::perform(load_visibility(), NiriWindowMessage::SwitchHeatLoaded)
    }

    pub(super) fn toggle(&self) -> IconToggle {
        IconToggle::new(
            "Switch heat",
            Phosphor::Fire(Weight::Regular),
            self.heat_visibility,
            SwitchPresses {
                turning_on: "tint each window's border warmer the more often you return to it, \
                             and ring the focused window",
                turning_off: "keep niri's own borders and focus ring",
            },
        )
    }

    pub(super) fn flip(&mut self) -> Task<NiriWindowMessage> {
        self.heat_visibility = self.heat_visibility.toggled();
        logged_failure_task(
            "switch heat borders: save".to_owned(),
            save_visibility(self.heat_visibility),
        )
    }

    pub(super) fn adopt_loaded(&mut self, heat_visibility: SwitchHeatVisibility) {
        self.heat_visibility = heat_visibility;
    }

    pub(super) fn reheat(
        &self,
        rules_file: &mut NiriRulesFile,
        styling: NiriStyling,
        palette: &Palette,
        heated_windows: impl FnOnce() -> Vec<HeatedWindow>,
    ) -> Result<SaveStep, RulesSaveError> {
        let switch_heat = match self.heat_visibility {
            SwitchHeatVisibility::Shown => SwitchHeat::Shown {
                focus_ring_color: format!("{:x}", palette.primary.to_rgba8()),
                neutral_border_color: format!("{:x}", palette.background.to_rgba8()),
                windows: heated_windows(),
            },
            SwitchHeatVisibility::Hidden => SwitchHeat::Hidden,
        };
        rules_file.reheat(styling, switch_heat)
    }
}

async fn load_visibility() -> SwitchHeatVisibility {
    with_table(
        "switch heat borders: load failed",
        &[CREATE_TABLE],
        read_visibility,
    )
    .await
    .unwrap_or_default()
}

async fn save_visibility(heat_visibility: SwitchHeatVisibility) -> Result<(), StoreError> {
    in_table(&[CREATE_TABLE], async |connection| {
        upsert_column(
            connection,
            "switch_heat_borders",
            "shown",
            flag(heat_visibility == SwitchHeatVisibility::Shown),
        )
        .await
    })
    .await
}

async fn read_visibility(
    connection: &turso::Connection,
) -> Result<SwitchHeatVisibility, StoreError> {
    let mut rows = connection
        .query("SELECT shown FROM switch_heat_borders WHERE id = 0", ())
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(SwitchHeatVisibility::default());
    };
    Ok(match columns.optional_integer(&row, "shown") {
        Some(0) => SwitchHeatVisibility::Hidden,
        Some(_) | None => SwitchHeatVisibility::Shown,
    })
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::sqlite_state_store::{
        flag, in_memory_connection, upsert_column,
    };

    use super::{CREATE_TABLE, SwitchHeatVisibility, read_visibility};

    #[tokio::test]
    async fn borders_show_until_turned_off() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        assert_eq!(
            read_visibility(&connection).await.unwrap(),
            SwitchHeatVisibility::Shown
        );
        upsert_column(&connection, "switch_heat_borders", "shown", flag(false))
            .await
            .unwrap();
        assert_eq!(
            read_visibility(&connection).await.unwrap(),
            SwitchHeatVisibility::Hidden
        );
    }
}
