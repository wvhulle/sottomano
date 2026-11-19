use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use crate::desktop_notifications::spec_hints::hint_str;

const STACK_TAG_HINTS: [&str; 2] = ["x-dunst-stack-tag", "x-canonical-private-synchronous"];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StackTag {
    Tagged(String),
    #[default]
    Untagged,
}

#[must_use]
pub fn stack_tag(hints: &HashMap<String, OwnedValue>) -> StackTag {
    STACK_TAG_HINTS
        .iter()
        .find_map(|hint_name| hint_str(hints, hint_name))
        .map_or(StackTag::Untagged, StackTag::Tagged)
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::Str;

    use super::*;

    #[test]
    fn the_dunst_tag_outranks_the_canonical_one() {
        let hints = HashMap::from([
            (
                "x-canonical-private-synchronous".to_owned(),
                OwnedValue::from(Str::from("volume-old")),
            ),
            (
                "x-dunst-stack-tag".to_owned(),
                OwnedValue::from(Str::from("volume")),
            ),
        ]);
        assert_eq!(stack_tag(&hints), StackTag::Tagged("volume".to_owned()));
    }

    #[test]
    fn no_tag_hint_leaves_the_notification_untagged() {
        assert_eq!(stack_tag(&HashMap::new()), StackTag::Untagged);
    }
}
