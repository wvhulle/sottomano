use crate::{
    data::severity::Severity, wayland_layers::tooltip::HoverTarget,
    widgets::reading_alert::ReadingAlert,
};

pub struct ShownReadingAlertToast<'a> {
    pub toast_id: u32,
    pub hover_target: &'a HoverTarget,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub struct DismissedReadingAlert {
    pub hover_target: HoverTarget,
    pub dismissed_at_severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSlotForAlert {
    ReplaceShownToast(u32),
    OpenNewToast,
}

pub struct ReadingAlertToShow<Message> {
    pub alert: ReadingAlert<Message>,
    pub toast_slot: ToastSlotForAlert,
}

pub struct ReadingAlertToastChanges<Message> {
    pub alerts_to_show: Vec<ReadingAlertToShow<Message>>,
    pub toast_ids_to_close: Vec<u32>,
    pub dismissals_to_forget: Vec<HoverTarget>,
}

impl<Message> ReadingAlertToastChanges<Message> {
    pub fn is_empty(&self) -> bool {
        self.alerts_to_show.is_empty()
            && self.toast_ids_to_close.is_empty()
            && self.dismissals_to_forget.is_empty()
    }
}

fn alert_for<'a, Message>(
    active_alerts: &'a [ReadingAlert<Message>],
    hover_target: &HoverTarget,
) -> Option<&'a ReadingAlert<Message>> {
    active_alerts
        .iter()
        .find(|alert| alert.hover_target == *hover_target)
}

fn toast_slot_for<Message>(
    alert: &ReadingAlert<Message>,
    shown_toasts: &[ShownReadingAlertToast<'_>],
) -> Option<ToastSlotForAlert> {
    match shown_toasts
        .iter()
        .find(|shown| *shown.hover_target == alert.hover_target)
    {
        Some(shown) if shown.severity >= alert.severity => None,
        Some(shown) => Some(ToastSlotForAlert::ReplaceShownToast(shown.toast_id)),
        None => Some(ToastSlotForAlert::OpenNewToast),
    }
}

fn still_dismissed<Message>(
    alert: &ReadingAlert<Message>,
    dismissed_alerts: &[DismissedReadingAlert],
) -> bool {
    dismissed_alerts.iter().any(|dismissed| {
        dismissed.hover_target == alert.hover_target
            && alert.severity <= dismissed.dismissed_at_severity
    })
}

pub fn plan_reading_alert_toast_changes<Message: Clone>(
    active_alerts: &[ReadingAlert<Message>],
    shown_toasts: &[ShownReadingAlertToast<'_>],
    dismissed_alerts: &[DismissedReadingAlert],
) -> ReadingAlertToastChanges<Message> {
    ReadingAlertToastChanges {
        alerts_to_show: active_alerts
            .iter()
            .filter(|alert| !still_dismissed(alert, dismissed_alerts))
            .filter_map(|alert| {
                toast_slot_for(alert, shown_toasts).map(|toast_slot| ReadingAlertToShow {
                    alert: alert.clone(),
                    toast_slot,
                })
            })
            .collect(),
        toast_ids_to_close: shown_toasts
            .iter()
            .filter(|shown| alert_for(active_alerts, shown.hover_target).is_none())
            .map(|shown| shown.toast_id)
            .collect(),
        dismissals_to_forget: dismissed_alerts
            .iter()
            .filter(|dismissed| {
                alert_for(active_alerts, &dismissed.hover_target)
                    .is_none_or(|alert| alert.severity > dismissed.dismissed_at_severity)
            })
            .map(|dismissed| dismissed.hover_target.clone())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(severity: Severity) -> ReadingAlert<()> {
        ReadingAlert {
            hover_target: HoverTarget::BatteryTime,
            severity,
            summary_text: "",
            body_text: String::new(),
            on_body_clicked: (),
            action_buttons: Vec::new(),
        }
    }

    fn shown(severity: Severity) -> ShownReadingAlertToast<'static> {
        ShownReadingAlertToast {
            toast_id: 7,
            hover_target: &HoverTarget::BatteryTime,
            severity,
        }
    }

    fn dismissed(dismissed_at_severity: Severity) -> DismissedReadingAlert {
        DismissedReadingAlert {
            hover_target: HoverTarget::BatteryTime,
            dismissed_at_severity,
        }
    }

    fn slots(changes: &ReadingAlertToastChanges<()>) -> Vec<ToastSlotForAlert> {
        changes
            .alerts_to_show
            .iter()
            .map(|to_show| to_show.toast_slot)
            .collect()
    }

    #[test]
    fn a_new_alert_opens_a_toast() {
        let changes = plan_reading_alert_toast_changes(&[active(Severity::Warning)], &[], &[]);
        assert_eq!(slots(&changes), [ToastSlotForAlert::OpenNewToast]);
    }

    #[test]
    fn a_shown_alert_at_the_same_level_changes_nothing() {
        let changes = plan_reading_alert_toast_changes(
            &[active(Severity::Warning)],
            &[shown(Severity::Warning)],
            &[],
        );
        assert!(changes.is_empty());
    }

    #[test]
    fn a_higher_level_replaces_the_shown_toast() {
        let changes = plan_reading_alert_toast_changes(
            &[active(Severity::Critical)],
            &[shown(Severity::Warning)],
            &[],
        );
        assert_eq!(slots(&changes), [ToastSlotForAlert::ReplaceShownToast(7)]);
    }

    #[test]
    fn a_toast_whose_alert_is_gone_closes() {
        let changes = plan_reading_alert_toast_changes::<()>(&[], &[shown(Severity::Warning)], &[]);
        assert_eq!(changes.toast_ids_to_close, [7]);
    }

    #[test]
    fn a_dismissed_alert_stays_dismissed_until_its_level_rises() {
        let same_level = plan_reading_alert_toast_changes(
            &[active(Severity::Warning)],
            &[],
            &[dismissed(Severity::Warning)],
        );
        assert!(same_level.is_empty());

        let risen = plan_reading_alert_toast_changes(
            &[active(Severity::Critical)],
            &[],
            &[dismissed(Severity::Warning)],
        );
        assert_eq!(slots(&risen), [ToastSlotForAlert::OpenNewToast]);
        assert_eq!(risen.dismissals_to_forget, [HoverTarget::BatteryTime]);
    }

    #[test]
    fn a_dismissal_is_forgotten_once_the_alert_is_gone() {
        let changes =
            plan_reading_alert_toast_changes::<()>(&[], &[], &[dismissed(Severity::Warning)]);
        assert_eq!(changes.dismissals_to_forget, [HoverTarget::BatteryTime]);
    }
}
