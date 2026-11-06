use super::split_app_title::SplitAppTitle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandedBrowser {
    LibreWolf,
    Chromium,
    GoogleChrome,
    Brave,
}

impl BrandedBrowser {
    #[must_use]
    pub fn of_app_id(app_id: &str) -> Option<Self> {
        match app_id {
            "librewolf" | "io.gitlab.librewolf-community" => Some(BrandedBrowser::LibreWolf),
            "chromium" | "chromium-browser" | "org.chromium.Chromium" => {
                Some(BrandedBrowser::Chromium)
            }
            "google-chrome" | "com.google.Chrome" => Some(BrandedBrowser::GoogleChrome),
            "brave-browser" | "com.brave.Browser" => Some(BrandedBrowser::Brave),
            _ => None,
        }
    }

    fn title_brand(self) -> &'static str {
        match self {
            BrandedBrowser::LibreWolf => "LibreWolf",
            BrandedBrowser::Chromium => "Chromium",
            BrandedBrowser::GoogleChrome => "Google Chrome",
            BrandedBrowser::Brave => "Brave",
        }
    }

    #[must_use]
    pub fn split_title(self, title: &str) -> Option<SplitAppTitle> {
        SplitAppTitle::before_brand(title, &[self.title_brand()])
    }
}

#[cfg(test)]
mod tests {
    use super::{super::split_app_title::in_context, *};

    #[test]
    fn branded_browser_titles_split_into_page_and_site() {
        assert_eq!(
            BrandedBrowser::Chromium.split_title("Odin-rs · GitHub - Chromium"),
            Some(in_context("Odin-rs", "GitHub"))
        );
        assert_eq!(
            BrandedBrowser::Brave.split_title("Odin-rs - Chromium"),
            None
        );
    }

    #[test]
    fn branded_browsers_are_recognised_by_exact_app_id() {
        assert_eq!(
            BrandedBrowser::of_app_id("org.chromium.Chromium"),
            Some(BrandedBrowser::Chromium)
        );
        assert_eq!(BrandedBrowser::of_app_id("chromium-like"), None);
    }
}
