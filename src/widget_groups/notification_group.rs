use crate::{
    config::Config,
    desktop_shell::hover_tooltips::HoverTarget,
    state::NotificationSettings,
    widget_groups::quiet_schedule_group::QuietScheduleTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        source_reading::source_reading,
        toggle_control::IconToggle,
        tooltip_body::{KeyValue, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Expansion(ExpansionMessage),
    ToggleSound,
    ToggleDoNotDisturb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationTarget {
    Sound,
}

impl From<NotificationTarget> for HoverTarget {
    fn from(target: NotificationTarget) -> Self {
        HoverTarget::Notification(target)
    }
}

#[derive(Default)]
pub struct NotificationGroup {
    expansion: GroupExpansion,
}

impl NotificationGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, message: ExpansionMessage) {
        self.expansion.update(message);
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        notification_settings: &NotificationSettings,
        live: usize,
        held: usize,
        quiet: bool,
    ) -> GroupContent<'a, NotificationGroupMessage> {
        let status = NotificationStatus { quiet, live, held };
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
                    source_reading(config, &status, reveal).map(GroupEvent::Group)
                }),
                GroupItem::reading(
                    DoNotDisturbStatus { quiet },
                    QuietScheduleTarget::DoNotDisturb,
                )
                .on_press(
                    "start or end do not disturb now",
                    NotificationGroupMessage::ToggleDoNotDisturb,
                ),
                GroupItem::toggle(
                    IconToggle::new(
                        "Notification sound",
                        sound_icon,
                        notification_settings.sound,
                    ),
                    NotificationTarget::Sound,
                    NotificationGroupMessage::ToggleSound,
                ),
            ],
        )
    }
}

#[derive(Clone, Copy)]
struct NotificationStatus {
    quiet: bool,
    live: usize,
    held: usize,
}

impl BarWidget for NotificationStatus {
    fn name(&self) -> &'static str {
        "Notifications"
    }

    fn icon(&self) -> Glyph {
        if self.quiet {
            Phosphor::ChatCircleSlash(self.state().weight()).into()
        } else {
            Phosphor::ChatCircle(self.state().weight()).into()
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.live + self.held)
            .ok()
            .filter(|count| *count > 0)
    }

    fn state(&self) -> WidgetState {
        if self.quiet {
            WidgetState::Off
        } else if self.live > 0 {
            WidgetState::On
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self) -> TooltipBody {
        if self.held > 0 {
            TooltipBody::dictionary([KeyValue::new(
                "Held until do not disturb ends",
                self.held.to_string(),
            )])
        } else {
            TooltipBody::sections([])
        }
    }
}

#[derive(Clone, Copy)]
struct DoNotDisturbStatus {
    quiet: bool,
}

impl BarWidget for DoNotDisturbStatus {
    fn name(&self) -> &'static str {
        "Do not disturb"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Prohibit(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.quiet)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([])
    }
}
