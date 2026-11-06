use super::{
    branded_browser_titles::BrandedBrowser,
    firefox_page_titles::{FIREFOX_APP_IDS, split_firefox_title},
    split_app_title::SplitAppTitle,
    thunderbird_mail_titles::{THUNDERBIRD_APP_IDS, split_thunderbird_title},
    visual_studio_code_titles::{VISUAL_STUDIO_CODE_APP_IDS, split_visual_studio_code_title},
    zed_editor_titles::{ZED_APP_IDS, split_zed_title},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognisedApp {
    Firefox,
    BrandedBrowser(BrandedBrowser),
    VisualStudioCode,
    Zed,
    Thunderbird,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognisedAppKind {
    Browser,
    Editor,
    Mail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnrecognisedApp;

impl TryFrom<&str> for RecognisedApp {
    type Error = UnrecognisedApp;

    fn try_from(app_id: &str) -> Result<Self, Self::Error> {
        if FIREFOX_APP_IDS.contains(&app_id) {
            return Ok(RecognisedApp::Firefox);
        }
        if VISUAL_STUDIO_CODE_APP_IDS.contains(&app_id) {
            return Ok(RecognisedApp::VisualStudioCode);
        }
        if ZED_APP_IDS.contains(&app_id) {
            return Ok(RecognisedApp::Zed);
        }
        if THUNDERBIRD_APP_IDS.contains(&app_id) {
            return Ok(RecognisedApp::Thunderbird);
        }
        BrandedBrowser::of_app_id(app_id)
            .map(RecognisedApp::BrandedBrowser)
            .ok_or(UnrecognisedApp)
    }
}

impl RecognisedApp {
    #[must_use]
    pub fn kind(self) -> RecognisedAppKind {
        match self {
            RecognisedApp::Firefox | RecognisedApp::BrandedBrowser(_) => RecognisedAppKind::Browser,
            RecognisedApp::VisualStudioCode | RecognisedApp::Zed => RecognisedAppKind::Editor,
            RecognisedApp::Thunderbird => RecognisedAppKind::Mail,
        }
    }

    #[must_use]
    pub fn split_title(self, title: &str) -> Option<SplitAppTitle> {
        match self {
            RecognisedApp::Firefox => split_firefox_title(title),
            RecognisedApp::BrandedBrowser(browser) => browser.split_title(title),
            RecognisedApp::VisualStudioCode => split_visual_studio_code_title(title),
            RecognisedApp::Zed => split_zed_title(title),
            RecognisedApp::Thunderbird => split_thunderbird_title(title),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_app_id_maps_to_its_own_title_parser() {
        assert_eq!(
            RecognisedApp::try_from("code"),
            Ok(RecognisedApp::VisualStudioCode)
        );
        assert_eq!(
            RecognisedApp::try_from("dev.zed.Zed"),
            Ok(RecognisedApp::Zed)
        );
        assert_eq!(
            RecognisedApp::try_from("thunderbird"),
            Ok(RecognisedApp::Thunderbird)
        );
        assert_eq!(
            RecognisedApp::try_from("com.brave.Browser"),
            Ok(RecognisedApp::BrandedBrowser(BrandedBrowser::Brave))
        );
        assert_eq!(
            RecognisedApp::try_from("org.kde.dolphin"),
            Err(UnrecognisedApp)
        );
    }
}
