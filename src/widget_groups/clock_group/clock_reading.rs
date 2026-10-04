use iced::{Alignment, Element, widget::row};

use crate::{
    config::Config,
    desktop_shell::hover_tooltips::{HoverTarget, hoverable},
    widgets::{
        bar_widget::Glyph, group_item::GroupItem, source_reading::icon_value_gap, text::bar_text,
        widget_group::GroupEvent, widget_state::WidgetState,
    },
};

#[derive(Clone)]
pub struct ClockReading {
    pub icon: Glyph,
    pub label: String,
    pub target: HoverTarget,
}

impl ClockReading {
    pub fn item<'a, M: 'a>(self, config: &'a Config) -> GroupItem<'a, M> {
        GroupItem::hoverable_element(move |reveal| self.clone().view(config, reveal))
    }

    fn view<'a, M: 'a>(self, config: &Config, reveal: f32) -> Element<'a, GroupEvent<M>> {
        let color = WidgetState::Idle.color(&config.palette).scale_alpha(reveal);
        hoverable(
            row![
                self.icon.view(config.font.icon_size_px, color),
                bar_text(&config.font, self.label).color(color),
            ]
            .spacing(icon_value_gap(config))
            .align_y(Alignment::Center)
            .into(),
            self.target,
            GroupEvent::Tooltip,
        )
    }
}
