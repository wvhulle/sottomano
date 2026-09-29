use std::hash::{DefaultHasher, Hash, Hasher};

use crate::{
    data::notifications::model::{ExpireTimeout, NotificationData, Urgency},
    wayland_layers::{notifications::NotificationMessage, tooltip::HoverTarget},
    widgets::{bar_widget::BarWidget, widget_state::Level},
};

const ALERT_ID_RANGE: u32 = 1 << 31;

#[allow(clippy::cast_possible_truncation)]
fn notification_id(target: &HoverTarget) -> u32 {
    let mut hasher = DefaultHasher::new();
    target.hash(&mut hasher);
    hasher.finish() as u32 | ALERT_ID_RANGE
}

const fn urgency_of(level: Level) -> Urgency {
    match level {
        Level::Ok => Urgency::Low,
        Level::Warning => Urgency::Warning,
        Level::Critical => Urgency::Critical,
    }
}

pub fn severity_of(reading: Option<&dyn BarWidget>) -> Level {
    reading.map_or(Level::Ok, |reading| reading.state().severity())
}

pub fn severity_change(
    target: &HoverTarget,
    summary: &str,
    before: Level,
    reading: Option<&dyn BarWidget>,
) -> Option<NotificationMessage> {
    let after = severity_of(reading);
    if after > before {
        return Some(NotificationMessage::Served(Box::new(NotificationData {
            id: notification_id(target),
            app_name: "vetrata".to_owned(),
            summary: summary.to_owned(),
            body: reading.and_then(BarWidget::label).unwrap_or_default(),
            urgency: urgency_of(after),
            expire_timeout: ExpireTimeout::Never,
            ..NotificationData::default()
        })));
    }
    (after == Level::Ok && before != Level::Ok)
        .then(|| NotificationMessage::CloseRequested(notification_id(target)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{
        bar_widget::Glyph,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        widget_state::WidgetState,
    };

    struct Fixed(Level);

    impl BarWidget for Fixed {
        fn icon(&self) -> Glyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn state(&self) -> WidgetState {
            self.0.into()
        }

        fn tooltip(&self) -> TooltipBody {
            TooltipBody::rows(Vec::new())
        }
    }

    fn change(before: Level, after: Level) -> Option<NotificationMessage> {
        severity_change(&HoverTarget::BatteryTime, "", before, Some(&Fixed(after)))
    }

    #[test]
    fn rising_severity_serves_a_persistent_toast_at_the_new_urgency() {
        let Some(NotificationMessage::Served(data)) = change(Level::Warning, Level::Critical) else {
            panic!("expected a served notification");
        };
        assert_eq!(data.urgency, Urgency::Critical);
        assert_eq!(data.expire_timeout, ExpireTimeout::Never);
    }

    #[test]
    fn recovering_to_ok_closes_the_same_toast() {
        let Some(NotificationMessage::Served(data)) = change(Level::Ok, Level::Warning) else {
            panic!("expected a served notification");
        };
        assert!(matches!(
            change(Level::Warning, Level::Ok),
            Some(NotificationMessage::CloseRequested(id)) if id == data.id
        ));
    }

    #[test]
    fn steady_or_easing_severity_sends_nothing() {
        assert!(change(Level::Ok, Level::Ok).is_none());
        assert!(change(Level::Warning, Level::Warning).is_none());
        assert!(change(Level::Critical, Level::Warning).is_none());
    }

    #[test]
    fn a_vanished_reading_counts_as_recovered() {
        assert!(matches!(
            severity_change(&HoverTarget::BatteryTime, "", Level::Warning, None),
            Some(NotificationMessage::CloseRequested(_))
        ));
    }
}
