use crate::{
    data::{
        notifications::model::{ExpireTimeout, NotifAction, NotificationData},
        severity::Severity,
    },
    wayland_layers::tooltip::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, ReadingAlertThreshold},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone)]
pub struct ReadingAlertActionButton<Message> {
    pub button_label: String,
    pub on_button_pressed: Message,
}

#[derive(Debug, Clone)]
pub struct ActiveReadingAlert<Message> {
    pub hover_target: HoverTarget,
    pub severity: Severity,
    pub summary_text: &'static str,
    pub body_text: String,
    pub on_body_clicked: Message,
    pub action_buttons: Vec<ReadingAlertActionButton<Message>>,
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

impl<Message> ActiveReadingAlert<Message> {
    pub fn from_widget_state(
        hover_target: HoverTarget,
        widget: Option<&dyn BarWidget>,
        on_body_clicked: Message,
    ) -> Option<Self> {
        let widget = widget?;
        let ReadingAlertThreshold::NotifyFromSeverity {
            minimum_severity,
            summary_text,
        } = widget.reading_alert_threshold()
        else {
            return None;
        };
        let severity = alerting_severity_of(widget.state());
        (severity > Severity::Normal && severity >= minimum_severity).then(|| Self {
            hover_target,
            severity,
            summary_text,
            body_text: widget.label().unwrap_or_default(),
            on_body_clicked,
            action_buttons: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_action_button(
        mut self,
        button_label: impl Into<String>,
        on_button_pressed: Message,
    ) -> Self {
        self.action_buttons.push(ReadingAlertActionButton {
            button_label: button_label.into(),
            on_button_pressed,
        });
        self
    }

    pub fn map_messages<Lifted>(
        self,
        lift: impl Fn(Message) -> Lifted,
    ) -> ActiveReadingAlert<Lifted> {
        ActiveReadingAlert {
            hover_target: self.hover_target,
            severity: self.severity,
            summary_text: self.summary_text,
            body_text: self.body_text,
            on_body_clicked: lift(self.on_body_clicked),
            action_buttons: self
                .action_buttons
                .into_iter()
                .map(|button| ReadingAlertActionButton {
                    button_label: button.button_label,
                    on_button_pressed: lift(button.on_button_pressed),
                })
                .collect(),
        }
    }

    pub fn action_button_pressed(&self, action_key: &str) -> Option<&Message> {
        let index: usize = action_key.parse().ok()?;
        self.action_buttons
            .get(index)
            .map(|button| &button.on_button_pressed)
    }

    pub fn toast_notification_data(&self, toast_id: u32) -> NotificationData {
        NotificationData {
            id: toast_id,
            app_name: "vetrata".to_owned(),
            summary: self.summary_text.to_owned(),
            body: self.body_text.clone(),
            actions: self
                .action_buttons
                .iter()
                .enumerate()
                .map(|(index, button)| NotifAction {
                    key: index.to_string(),
                    label: button.button_label.clone(),
                })
                .collect(),
            severity: self.severity,
            expire_timeout: ExpireTimeout::Never,
            ..NotificationData::default()
        }
    }
}

pub fn highest_active_alert_severity<Message>(alerts: &[ActiveReadingAlert<Message>]) -> Severity {
    alerts
        .iter()
        .map(|alert| alert.severity)
        .max()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{
        bar_widget::Glyph,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
    };

    struct FixedReading {
        state: WidgetState,
        threshold: ReadingAlertThreshold,
    }

    impl BarWidget for FixedReading {
        fn icon(&self) -> Glyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn state(&self) -> WidgetState {
            self.state
        }

        fn reading_alert_threshold(&self) -> ReadingAlertThreshold {
            self.threshold
        }

        fn tooltip(&self) -> TooltipBody {
            TooltipBody::rows(Vec::new())
        }
    }

    const FROM_WARNING: ReadingAlertThreshold = ReadingAlertThreshold::NotifyFromSeverity {
        minimum_severity: Severity::Warning,
        summary_text: "summary",
    };

    const FROM_CRITICAL: ReadingAlertThreshold = ReadingAlertThreshold::NotifyFromSeverity {
        minimum_severity: Severity::Critical,
        summary_text: "summary",
    };

    fn alert_for(
        state: WidgetState,
        threshold: ReadingAlertThreshold,
    ) -> Option<ActiveReadingAlert<u8>> {
        ActiveReadingAlert::from_widget_state(
            HoverTarget::BatteryTime,
            Some(&FixedReading { state, threshold }),
            0,
        )
    }

    #[test]
    fn widgets_that_never_notify_have_no_alert() {
        assert!(alert_for(WidgetState::Critical, ReadingAlertThreshold::NeverNotify).is_none());
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

    #[test]
    fn mapping_messages_keeps_the_body_message_and_the_action_buttons() {
        let alert = alert_for(WidgetState::Warning, FROM_WARNING)
            .expect("warning reading alerts")
            .with_action_button("first", 1)
            .with_action_button("second", 2)
            .map_messages(|message| u16::from(message) * 10);
        assert_eq!(alert.on_body_clicked, 0);
        assert_eq!(alert.action_button_pressed("1"), Some(&20));
        assert_eq!(alert.action_button_pressed("2"), None);
        assert_eq!(alert.toast_notification_data(7).actions[0].label, "first");
    }
}
