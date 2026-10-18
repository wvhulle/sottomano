const SEPARATORS: [&str; 3] = [" — ", " · ", " - "];

const FIREFOX_BRANDS: [&str; 2] = ["Mozilla Firefox", "Firefox"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPage {
    pub page: String,
    pub site: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBrand {
    LibreWolf,
    Chromium,
    GoogleChrome,
    Brave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnbrandedApp;

impl TryFrom<&str> for BrowserBrand {
    type Error = UnbrandedApp;

    fn try_from(app_id: &str) -> Result<Self, Self::Error> {
        match app_id {
            "librewolf" | "io.gitlab.librewolf-community" => Ok(BrowserBrand::LibreWolf),
            "chromium" | "chromium-browser" | "org.chromium.Chromium" => Ok(BrowserBrand::Chromium),
            "google-chrome" | "com.google.Chrome" => Ok(BrowserBrand::GoogleChrome),
            "brave-browser" | "com.brave.Browser" => Ok(BrowserBrand::Brave),
            _ => Err(UnbrandedApp),
        }
    }
}

impl BrowserBrand {
    fn title_suffix(self) -> &'static str {
        match self {
            BrowserBrand::LibreWolf => "LibreWolf",
            BrowserBrand::Chromium => "Chromium",
            BrowserBrand::GoogleChrome => "Google Chrome",
            BrowserBrand::Brave => "Brave",
        }
    }
}

impl BrowserPage {
    #[must_use]
    pub fn in_firefox(title: &str) -> Option<Self> {
        Self::before_brand(title, &FIREFOX_BRANDS)
    }

    #[must_use]
    pub fn in_branded_browser(title: &str, brand: BrowserBrand) -> Option<Self> {
        Self::before_brand(title, &[brand.title_suffix()])
    }

    fn before_brand(title: &str, brands: &[&str]) -> Option<Self> {
        let page = page_title(title, brands).filter(|page| !page.is_empty())?;
        let Some(site_separator) = last_separator(page) else {
            return Some(BrowserPage {
                page: page.to_owned(),
                site: None,
            });
        };
        Some(BrowserPage {
            page: page[..site_separator.at].to_owned(),
            site: Some(page[site_separator.at + site_separator.length..].to_owned()),
        })
    }
}

#[must_use]
pub fn firefox_favicon_key(title: &str) -> Option<String> {
    let without_badge = strip_unread_badge(page_title(title, &FIREFOX_BRANDS)?);
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
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return title;
    }
    tail
}

fn page_title<'a>(title: &'a str, brands: &[&str]) -> Option<&'a str> {
    brands.iter().find_map(|brand| {
        SEPARATORS
            .iter()
            .find_map(|separator| title.strip_suffix(&format!("{separator}{brand}")))
    })
}

struct SeparatorMatch {
    at: usize,
    length: usize,
}

fn last_separator(text: &str) -> Option<SeparatorMatch> {
    SEPARATORS
        .iter()
        .filter_map(|separator| {
            Some(SeparatorMatch {
                at: text.rfind(separator)?,
                length: separator.len(),
            })
        })
        .max_by_key(|found| found.at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firefox_titles_split_into_page_and_site() {
        assert_eq!(
            BrowserPage::in_firefox("Inbox — user@example.com — Mozilla Firefox"),
            Some(BrowserPage {
                page: "Inbox".to_owned(),
                site: Some("user@example.com".to_owned()),
            })
        );
        assert_eq!(BrowserPage::in_firefox("about:blank"), None);
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

    #[test]
    fn branded_browsers_are_recognised_by_exact_app_id() {
        assert_eq!(
            BrowserBrand::try_from("org.chromium.Chromium"),
            Ok(BrowserBrand::Chromium)
        );
        assert_eq!(BrowserBrand::try_from("chromium-like"), Err(UnbrandedApp));
    }
}
