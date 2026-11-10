use iced::Task;
use sottomano_desktop_sinks::compositor_window_requests::compositor_rule_files::{
    CompositorRulesFile, RulesSaveError, SaveStep,
};
use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_styling_model::{
        CompositorStyling, HeatedWindow, SwitchHeat,
    },
    sqlite_state_store::{
        Columns, StoreError, flag, in_state_store, upsert_column, with_state_store,
    },
};
use sottomano_toml_config::theme::palette::Palette;

use crate::{
    background_work::logged_failure_task,
    group_items::toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    widget_groups::compositor_styling_group::CompositorStylingMessage,
    widgets::phosphor::{Phosphor, Weight},
};

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
    pub(super) fn load() -> Task<CompositorStylingMessage> {
        Task::perform(
            load_visibility(),
            CompositorStylingMessage::SwitchHeatLoaded,
        )
    }

    pub(super) fn toggle(&self) -> IconToggle {
        IconToggle::new(
            "Switch heat",
            Phosphor::Fire(Weight::Regular),
            self.heat_visibility,
            SwitchPresses {
                turning_on: "tint each window's border warmer the more often you return to it, \
                             and ring the focused window",
                turning_off: "keep the compositor's own borders and focus ring",
            },
        )
    }

    pub(super) fn flip(&mut self) -> Task<CompositorStylingMessage> {
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
        rules_file: &mut CompositorRulesFile,
        styling: CompositorStyling,
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
    with_state_store("switch heat borders: load failed", read_visibility)
        .await
        .unwrap_or_default()
}

async fn save_visibility(heat_visibility: SwitchHeatVisibility) -> Result<(), StoreError> {
    in_state_store(async |connection| {
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
        flag, migrated_in_memory_store, upsert_column,
    };

    use super::{SwitchHeatVisibility, read_visibility};

    #[tokio::test]
    async fn borders_show_until_turned_off() {
        let connection = migrated_in_memory_store().await;
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
