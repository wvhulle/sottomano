use std::collections::HashMap;

use sottomano_toml_config::severity::Severity;
use uom::si::{f64::Ratio, ratio::percent};
use zbus::zvariant::OwnedValue;

use super::{NIRI_WINDOW_ID_HINT, WINDOW_ID_HINT};
use crate::{
    desktop_notifications::{
        notification_data::{ActionTint, NotifAction, NotificationProgress, ReplyPrompt},
        spec_hints::hint_str,
    },
    system_protocol_clients::dbus_desktop_buses::prop,
};

pub(super) fn parse_actions(actions: &[String]) -> Vec<NotifAction> {
    actions
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| NotifAction {
            key: pair[0].clone(),
            label: pair[1].clone(),
            tint: ActionTint::from(pair[0].as_str()),
        })
        .collect()
}

pub(super) fn parse_urgency(hints: &HashMap<String, OwnedValue>) -> Severity {
    match prop::<u8>(hints, "urgency") {
        Some(0) => Severity::Low,
        Some(2) => Severity::Critical,
        _ => Severity::Normal,
    }
}

pub(super) fn hinted_window_id(
    hints: &HashMap<String, OwnedValue>,
    parse_window_text: impl Fn(&str) -> Option<u64>,
) -> Option<u64> {
    hint_str(hints, WINDOW_ID_HINT)
        .and_then(|hint_text| parse_window_text(&hint_text))
        .or_else(|| hint_window_id(hints, WINDOW_ID_HINT))
        .or_else(|| hint_window_id(hints, NIRI_WINDOW_ID_HINT))
}

fn hint_window_id(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<u64> {
    let signed_window_id = prop::<i32>(hints, key)?;
    u64::try_from(signed_window_id)
        .inspect_err(|_| {
            tracing::warn!("notifications: hint {key} has negative window id {signed_window_id}");
        })
        .ok()
}

pub(super) fn hint_progress(hints: &HashMap<String, OwnedValue>) -> NotificationProgress {
    match prop::<i32>(hints, "value") {
        Some(percent_value) => NotificationProgress::Measured(clamped_progress(percent_value)),
        None => NotificationProgress::Unmeasured,
    }
}

fn clamped_progress(percent_value: i32) -> Ratio {
    Ratio::new::<percent>(f64::from(percent_value.clamp(0, 100)))
}

pub(super) fn reply_prompt(
    action: Option<&NotifAction>,
    hints: &HashMap<String, OwnedValue>,
) -> ReplyPrompt {
    let Some(action) = action else {
        return ReplyPrompt::NotOffered;
    };
    ReplyPrompt::Offered {
        placeholder: hint_str(hints, "x-kde-reply-placeholder-text")
            .unwrap_or_else(|| action.label.clone()),
        submit: hint_str(hints, "x-kde-reply-submit-button-text")
            .unwrap_or_else(|| "Send".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uom::si::{f64::Ratio, ratio::percent};
    use zbus::zvariant::{OwnedValue, Str};

    use super::*;

    fn decimal_window(hint_text: &str) -> Option<u64> {
        hint_text.parse().ok()
    }

    #[test]
    fn a_textual_window_hint_is_parsed_by_the_compositor() {
        let hints = HashMap::from([("x-window-id".to_owned(), text("0x2a"))]);
        let hexadecimal_window =
            |hint_text: &str| u64::from_str_radix(hint_text.trim_start_matches("0x"), 16).ok();
        assert_eq!(hinted_window_id(&hints, hexadecimal_window), Some(42));
    }

    #[test]
    fn an_integer_window_hint_is_taken_as_is() {
        let hints = HashMap::from([("x-window-id".to_owned(), OwnedValue::from(7_i32))]);
        assert_eq!(hinted_window_id(&hints, decimal_window), Some(7));
    }

    #[test]
    fn the_niri_window_hint_is_still_understood() {
        let hints = HashMap::from([("x-niri-window-id".to_owned(), OwnedValue::from(9_i32))]);
        assert_eq!(hinted_window_id(&hints, decimal_window), Some(9));
    }

    #[test]
    fn an_unparsable_or_negative_window_hint_names_no_window() {
        let textual = HashMap::from([("x-window-id".to_owned(), text("kitty"))]);
        let negative = HashMap::from([("x-window-id".to_owned(), OwnedValue::from(-1_i32))]);
        assert_eq!(hinted_window_id(&textual, decimal_window), None);
        assert_eq!(hinted_window_id(&negative, decimal_window), None);
    }

    #[test]
    fn progress_hint_is_clamped_to_percent_range() {
        let percent_of = |value| Ratio::new::<percent>(value);
        assert_eq!(clamped_progress(-5), percent_of(0.0));
        assert_eq!(clamped_progress(42), percent_of(42.0));
        assert_eq!(clamped_progress(250), percent_of(100.0));
    }

    fn reply_action() -> NotifAction {
        NotifAction {
            key: "inline-reply".to_owned(),
            label: "Reply".to_owned(),
            tint: ActionTint::Proceed,
        }
    }

    fn text(value: &str) -> OwnedValue {
        OwnedValue::from(Str::from(value.to_owned()))
    }

    #[test]
    fn reply_without_inline_reply_action_is_not_offered() {
        assert_eq!(reply_prompt(None, &HashMap::new()), ReplyPrompt::NotOffered);
    }

    #[test]
    fn reply_falls_back_to_action_label_and_send() {
        assert_eq!(
            reply_prompt(Some(&reply_action()), &HashMap::new()),
            ReplyPrompt::Offered {
                placeholder: "Reply".to_owned(),
                submit: "Send".to_owned(),
            }
        );
    }

    #[test]
    fn reply_takes_kde_hint_texts() {
        let hints = HashMap::from([
            (
                "x-kde-reply-placeholder-text".to_owned(),
                text("Type an SMS"),
            ),
            (
                "x-kde-reply-submit-button-text".to_owned(),
                text("Send SMS"),
            ),
        ]);
        assert_eq!(
            reply_prompt(Some(&reply_action()), &hints),
            ReplyPrompt::Offered {
                placeholder: "Type an SMS".to_owned(),
                submit: "Send SMS".to_owned(),
            }
        );
    }
}
