use iced::widget::Row;

use crate::{
    config::Config,
    state::ToastPreferences,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        source_reading::source_reading,
        toggle_control::icon_toggle_glyph,
        tooltip_body::TooltipBody,
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationToggle {
    DoNotDisturb,
    Sound,
    SpatialLayout,
}

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Expansion(ExpansionMessage),
    Toggle(NotificationToggle),
}

#[derive(Default)]
pub struct NotificationGroup {
    pub(crate) expansion: GroupExpansion,
}

impl NotificationGroup {
    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        toast_preferences: &ToastPreferences,
        live: usize,
    ) -> WidgetGroup<'a, NotificationGroupMessage> {
        let status = NotificationStatus {
            dnd: toast_preferences.do_not_disturb,
            live,
        };
        let sound_icon = if toast_preferences.sound {
            Phosphor::Bell(Weight::Regular)
        } else {
            Phosphor::BellSlash(Weight::Regular)
        };
        let spatial_layout_icon = if toast_preferences.spatial_layout {
            Phosphor::TextAlignCenter(Weight::Regular)
        } else {
            Phosphor::TextAlignRight(Weight::Regular)
        };
        WidgetGroup::new(&self.expansion, NotificationGroupMessage::Expansion)
            .item(GroupItem::element(move |reveal| {
                source_reading(config, Some(&status), reveal).unwrap_or_else(|| Row::new().into())
            }))
            .item(toggle_item(
                config,
                Phosphor::Prohibit(Weight::Regular),
                toast_preferences.do_not_disturb,
                NotificationToggle::DoNotDisturb,
            ))
            .item(toggle_item(
                config,
                sound_icon,
                toast_preferences.sound,
                NotificationToggle::Sound,
            ))
            .item(toggle_item(
                config,
                spatial_layout_icon,
                toast_preferences.spatial_layout,
                NotificationToggle::SpatialLayout,
            ))
    }
}

#[derive(Clone, Copy)]
struct NotificationStatus {
    dnd: bool,
    live: usize,
}

impl BarWidget for NotificationStatus {
    fn icon(&self) -> Glyph {
        if self.dnd {
            Phosphor::ChatCircleSlash(self.state().weight()).into()
        } else {
            Phosphor::ChatCircle(self.state().weight()).into()
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.live).ok().filter(|live| *live > 0)
    }

    fn state(&self) -> WidgetState {
        if self.dnd {
            WidgetState::Off
        } else if self.live > 0 {
            WidgetState::On
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
    }
}

fn toggle_item(
    config: &Config,
    icon: Phosphor,
    enabled: bool,
    toggle: NotificationToggle,
) -> GroupItem<'_, NotificationGroupMessage> {
    GroupItem::element(move |_| icon_toggle_glyph(config, icon, enabled))
        .on_press(NotificationGroupMessage::Toggle(toggle))
}
