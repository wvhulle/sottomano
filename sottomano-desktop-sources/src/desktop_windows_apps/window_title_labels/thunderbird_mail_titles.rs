use super::split_app_title::SplitAppTitle;

pub const THUNDERBIRD_APP_IDS: [&str; 3] = [
    "thunderbird",
    "org.mozilla.Thunderbird",
    "net.thunderbird.Thunderbird",
];

const THUNDERBIRD_BRANDS: [&str; 2] = ["Mozilla Thunderbird", "Thunderbird"];

#[must_use]
pub fn split_thunderbird_title(title: &str) -> Option<SplitAppTitle> {
    SplitAppTitle::before_brand(title, &THUNDERBIRD_BRANDS)
}

#[cfg(test)]
mod tests {
    use super::{super::split_app_title::in_context, *};

    #[test]
    fn a_folder_names_its_account() {
        assert_eq!(
            split_thunderbird_title("Inbox - Gmail - Mozilla Thunderbird"),
            Some(in_context("Inbox", "Gmail"))
        );
    }

    #[test]
    fn dialogs_carry_no_brand_and_stay_unsplit() {
        assert_eq!(split_thunderbird_title("5 Reminders"), None);
        assert_eq!(split_thunderbird_title("System settings"), None);
    }
}
