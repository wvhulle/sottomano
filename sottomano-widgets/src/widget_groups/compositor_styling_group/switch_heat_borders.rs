use iced::Task;
use sottomano_desktop_sinks::compositor_window_requests::compositor_rule_files::{
    CompositorRulesFile, RulesSaveError, SaveStep,
};
use sottomano_desktop_sources::compositor_window_ipc::compositor_styling_model::{
    CompositorStyling, HeatedWindow, SwitchHeat,
};
use sottomano_toml_config::theme::palette::Palette;

use crate::{
    background_work::logged_failure_task,
    group_items::toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    widget_groups::{
        compositor_styling_group::CompositorStylingMessage,
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
    },
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SwitchHeatVisibility {
    #[default]
    Shown,
    Hidden,
}

impl SingletonFlagState for SwitchHeatVisibility {
    const STORED_FLAG: SingletonFlag = SingletonFlag::SwitchHeatBorders;
    const STORED_AS_SET: Self = SwitchHeatVisibility::Shown;
    const STORED_AS_CLEARED: Self = SwitchHeatVisibility::Hidden;
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
            sqlite_singleton_flag::load(SwitchHeatVisibility::default()),
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
            sqlite_singleton_flag::save(self.heat_visibility),
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
