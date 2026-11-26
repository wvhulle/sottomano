use iced::Function;
use sottomano_toml_config::Config;

use super::{QuietScheduleGroup, QuietScheduleMessage, QuietSlider};
use crate::group_indicators::group_indicator::GroupIndicator;

impl QuietScheduleGroup {
    pub(super) fn quiet_slider_row<'view>(
        &'view self,
        config: &'view Config,
        slider: QuietSlider,
    ) -> GroupIndicator<'view> {
        GroupIndicator::slider(
            self.slider(slider),
            config,
            QuietScheduleMessage::Slider.with(slider),
        )
    }
}
