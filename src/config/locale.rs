use std::env;

use chrono::Locale;

pub fn from_environment() -> Locale {
    ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .filter_map(|variable| env::var(variable).ok())
        .find(|value| !value.is_empty())
        .and_then(|value| without_codeset(&value).parse().ok())
        .unwrap_or_default()
}

fn without_codeset(locale: &str) -> String {
    let modifier = locale.find('@').map_or("", |at| &locale[at..]);
    let name = locale.split(['.', '@']).next().unwrap_or(locale);
    format!("{name}{modifier}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codeset_is_dropped_and_modifier_kept() {
        assert_eq!(without_codeset("en_US.UTF-8"), "en_US");
        assert_eq!(without_codeset("aa_ER.UTF-8@saaho"), "aa_ER@saaho");
        assert_eq!(without_codeset("nl_BE"), "nl_BE");
    }

    #[test]
    fn stripped_names_parse_as_locales() {
        assert_eq!(
            without_codeset("nl_BE.UTF-8").parse::<Locale>().ok(),
            Some(Locale::nl_BE)
        );
    }
}
