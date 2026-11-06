use super::split_app_title::{SplitAppTitle, title_without_brand};

pub const FIREFOX_APP_IDS: [&str; 2] = ["firefox", "org.mozilla.firefox"];

const FIREFOX_BRANDS: [&str; 2] = ["Mozilla Firefox", "Firefox"];

#[must_use]
pub fn split_firefox_title(title: &str) -> Option<SplitAppTitle> {
    SplitAppTitle::before_brand(title, &FIREFOX_BRANDS)
}

#[must_use]
pub fn firefox_favicon_key(title: &str) -> Option<String> {
    let without_badge = strip_unread_badge(title_without_brand(title, &FIREFOX_BRANDS)?);
    (!without_badge.is_empty()).then(|| without_badge.to_owned())
}

#[must_use]
pub fn strip_unread_badge(title: &str) -> &str {
    let Some(rest) = title.strip_prefix('(') else {
        return title;
    };
    let Some((count, tail)) = rest.split_once(") ") else {
        return title;
    };
    let digits = count.strip_suffix('+').unwrap_or(count);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return title;
    }
    tail
}

#[cfg(test)]
mod tests {
    use super::{super::split_app_title::in_context, *};

    #[test]
    fn firefox_titles_split_into_page_and_site() {
        assert_eq!(
            split_firefox_title("Inbox — user@example.com — Mozilla Firefox"),
            Some(in_context("Inbox", "user@example.com"))
        );
        assert_eq!(split_firefox_title("about:blank"), None);
    }

    #[test]
    fn firefox_favicon_key_ignores_unread_badges() {
        assert_eq!(
            firefox_favicon_key("(2) WhatsApp — Mozilla Firefox"),
            Some("WhatsApp".to_owned())
        );
        assert_eq!(
            firefox_favicon_key("(20+) Facebook — Firefox"),
            Some("Facebook".to_owned())
        );
        assert_eq!(
            firefox_favicon_key("(draft) Notes — Firefox"),
            Some("(draft) Notes".to_owned())
        );
        assert_eq!(firefox_favicon_key("some plain title"), None);
    }
}
