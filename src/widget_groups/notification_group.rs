use iced::widget::Row;

use crate::{
    config::Config,
    desktop_shell::hover_tooltips::HoverTarget,
    state::NotificationSettings,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        source_reading::source_reading,
        toggle_control::IconToggle,
        tooltip_body::TooltipBody,
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationToggle {
    DoNotDisturb,
    Sound,
}

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Expansion(ExpansionMessage),
    Toggle(NotificationToggle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationTarget {
    DoNotDisturb,
    Sound,
}

impl From<NotificationTarget> for HoverTarget {
    fn from(target: NotificationTarget) -> Self {
        HoverTarget::Notification(target)
    }
}

#[derive(Default)]
pub struct NotificationGroup {
    pub(crate) expansion: GroupExpansion,
}

impl NotificationGroup {
    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        notification_settings: &NotificationSettings,
        live: usize,
    ) -> GroupContent<'a, NotificationGroupMessage> {
        let status = NotificationStatus {
            dnd: notification_settings.do_not_disturb,
            live,
        };
        let sound_icon = if notification_settings.sound {
            Phosphor::Bell(Weight::Regular)
        } else {
            Phosphor::BellSlash(Weight::Regular)
        };
        GroupContent::new(
            &self.expansion,
            NotificationGroupMessage::Expansion,
            [
                GroupItem::element(move |reveal| {
                    source_reading(config, Some(&status), reveal)
                        .unwrap_or_else(|| Row::new().into())
                        .map(GroupEvent::Group)
                }),
                GroupItem::toggle(
                    IconToggle::new(
                        Phosphor::Prohibit(Weight::Regular),
                        notification_settings.do_not_disturb,
                    ),
                    NotificationTarget::DoNotDisturb,
                    NotificationGroupMessage::Toggle(NotificationToggle::DoNotDisturb),
                ),
                GroupItem::toggle(
                    IconToggle::new(sound_icon, notification_settings.sound),
                    NotificationTarget::Sound,
                    NotificationGroupMessage::Toggle(NotificationToggle::Sound),
                ),
            ],
        )
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
        TooltipBody::sections(&[])
    }
}
