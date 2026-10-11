use std::{borrow::Cow, time::SystemTime};

use chrono::Locale;

use crate::{
    data::{
        notifications::model::{ActionTint, ExpireTimeout, NotifAction, NotificationData},
        severity::Severity,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, OngoingToastThreshold},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone)]
pub struct OngoingToastActionButton<Message> {
    pub button_label: String,
    pub tint: ActionTint,
    pub on_button_pressed: Message,
}

impl<Message> OngoingToastActionButton<Message> {
    pub fn map_message<Lifted>(
        self,
        lift: impl Fn(Message) -> Lifted,
    ) -> OngoingToastActionButton<Lifted> {
        OngoingToastActionButton {
            button_label: self.button_label,
            tint: self.tint,
            on_button_pressed: lift(self.on_button_pressed),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OngoingKey {
    Reading(HoverTarget),
    ConfigFile,
}

#[derive(Debug, Clone)]
pub struct OngoingToast<Message> {
    pub widget_name: &'static str,
    pub key: OngoingKey,
    pub severity: Severity,
    pub summary_text: Cow<'static, str>,
    pub body_text: Option<String>,
    pub since: SystemTime,
    pub on_body_clicked: Message,
    pub action_buttons: Vec<OngoingToastActionButton<Message>>,
}

const fn alerting_severity_of(state: WidgetState) -> Severity {
    match state {
        WidgetState::Warning => Severity::Warning,
        WidgetState::Critical => Severity::Critical,
        WidgetState::Failed
        | WidgetState::Off
        | WidgetState::Idle
        | WidgetState::Pending
        | WidgetState::On => Severity::Normal,
    }
}

impl<Message> OngoingToast<Message> {
    pub fn from_widget_state(
        hover_target: HoverTarget,
        widget: Option<&dyn BarWidget>,
        on_body_clicked: Message,
        locale: Locale,
    ) -> Option<Self> {
        let widget = widget?;
        let OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity,
            summary_text,
        } = widget.ongoing_toast_threshold()
        else {
            return None;
        };
        let severity = alerting_severity_of(widget.state());
        (severity > Severity::Normal && severity >= minimum_severity).then(|| Self {
            widget_name: widget.name(),
            key: OngoingKey::Reading(hover_target),
            severity,
            summary_text: Cow::Borrowed(summary_text),
            body_text: widget.localized_label(locale),
            since: SystemTime::now(),
            on_body_clicked,
            action_buttons: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_action_button(mut self, button: OngoingToastActionButton<Message>) -> Self {
        self.action_buttons.push(button);
        self
    }

    pub fn map_messages<Lifted>(self, lift: impl Fn(Message) -> Lifted) -> OngoingToast<Lifted> {
        OngoingToast {
            widget_name: self.widget_name,
            key: self.key,
            severity: self.severity,
            summary_text: self.summary_text,
            body_text: self.body_text,
            since: self.since,
            on_body_clicked: lift(self.on_body_clicked),
            action_buttons: self
                .action_buttons
                .into_iter()
                .map(|button| button.map_message(&lift))
                .collect(),
        }
    }

    pub fn toast_notification_data(&self, toast_id: u32) -> NotificationData {
        NotificationData {
            body: self.body_text.clone(),
            actions: self
                .action_buttons
                .iter()
                .enumerate()
                .map(|(index, button)| NotifAction {
                    key: index.to_string(),
                    label: button.button_label.clone(),
                    tint: button.tint,
                })
                .collect(),
            severity: self.severity,
            expire_timeout: ExpireTimeout::Never,
            app_name: Some(self.widget_name.to_owned()),
            ..NotificationData::from_shell(toast_id, self.summary_text.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        widget_groups::power_group::PowerTarget,
        widgets::{
            bar_widget::Glyph,
            phosphor::{Phosphor, Weight},
            tooltip_body::TooltipBody,
        },
    };

    struct FixedReading {
        state: WidgetState,
        threshold: OngoingToastThreshold,
    }

    impl BarWidget for FixedReading {
        fn name(&self) -> &'static str {
            "Fixed reading"
        }

        fn icon(&self) -> Glyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn state(&self) -> WidgetState {
            self.state
        }

        fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
            self.threshold
        }

        fn tooltip(&self) -> TooltipBody {
            TooltipBody::sections(&[])
        }
    }

    const FROM_WARNING: OngoingToastThreshold = OngoingToastThreshold::NotifyFromSeverity {
        minimum_severity: Severity::Warning,
        summary_text: "summary",
    };

    const FROM_CRITICAL: OngoingToastThreshold = OngoingToastThreshold::NotifyFromSeverity {
        minimum_severity: Severity::Critical,
        summary_text: "summary",
    };

    fn alert_for(state: WidgetState, threshold: OngoingToastThreshold) -> Option<OngoingToast<u8>> {
        OngoingToast::from_widget_state(
            HoverTarget::Power(PowerTarget::BatteryTime),
            Some(&FixedReading { state, threshold }),
            0,
            Locale::default(),
        )
    }

    #[test]
    fn widgets_that_never_notify_have_no_alert() {
        assert!(alert_for(WidgetState::Critical, OngoingToastThreshold::NeverNotify).is_none());
    }

    #[test]
    fn readings_below_the_minimum_level_have_no_alert() {
        assert!(alert_for(WidgetState::Idle, FROM_WARNING).is_none());
        assert!(alert_for(WidgetState::Warning, FROM_CRITICAL).is_none());
        assert_eq!(
            alert_for(WidgetState::Critical, FROM_CRITICAL).map(|alert| alert.severity),
            Some(Severity::Critical)
        );
    }

    #[test]
    fn failed_readings_never_alert() {
        assert!(alert_for(WidgetState::Failed, FROM_WARNING).is_none());
    }

    fn proceed_button(button_label: &str, on_button_pressed: u8) -> OngoingToastActionButton<u8> {
        OngoingToastActionButton {
            button_label: button_label.to_owned(),
            tint: ActionTint::Proceed,
            on_button_pressed,
        }
    }

    #[test]
    fn mapping_messages_keeps_the_body_message_and_the_action_buttons() {
        let alert = alert_for(WidgetState::Warning, FROM_WARNING)
            .expect("warning reading alerts")
            .with_action_button(proceed_button("first", 1))
            .with_action_button(proceed_button("second", 2))
            .map_messages(|message| u16::from(message) * 10);
        assert_eq!(alert.on_body_clicked, 0);
        assert_eq!(alert.action_buttons[1].on_button_pressed, 20);
        assert_eq!(alert.toast_notification_data(7).actions[0].label, "first");
    }
}
