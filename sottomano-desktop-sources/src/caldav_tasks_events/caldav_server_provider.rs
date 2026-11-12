use reqwest::Url;

use crate::caldav_tasks_events::nextcloud_web_links::NextcloudWebRoot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaldavProvider {
    Nextcloud(NextcloudWebRoot),
    Icloud,
    GoogleCalendar,
    Zoho,
    MailboxOrg,
    Unrecognised,
}

const HOSTED_PROVIDER_DOMAINS: [(&str, CaldavProvider); 6] = [
    ("icloud.com", CaldavProvider::Icloud),
    ("google.com", CaldavProvider::GoogleCalendar),
    ("googleusercontent.com", CaldavProvider::GoogleCalendar),
    ("zoho.com", CaldavProvider::Zoho),
    ("zoho.eu", CaldavProvider::Zoho),
    ("mailbox.org", CaldavProvider::MailboxOrg),
];

impl CaldavProvider {
    #[must_use]
    pub fn detected<'a>(
        server_url: &str,
        calendar_hrefs: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        calendar_hrefs
            .into_iter()
            .find_map(|calendar_href| NextcloudWebRoot::detected(server_url, calendar_href))
            .map(CaldavProvider::Nextcloud)
            .or_else(|| hosted_provider(server_url))
            .unwrap_or(CaldavProvider::Unrecognised)
    }

    #[must_use]
    pub fn nextcloud_web_root(&self) -> Option<&NextcloudWebRoot> {
        match self {
            CaldavProvider::Nextcloud(web_root) => Some(web_root),
            _ => None,
        }
    }
}

fn hosted_provider(server_url: &str) -> Option<CaldavProvider> {
    let url = Url::parse(server_url)
        .inspect_err(|parse_failure| {
            tracing::warn!("caldav: server url {server_url} is malformed: {parse_failure}");
        })
        .ok()?;
    let host = url.host_str()?;
    HOSTED_PROVIDER_DOMAINS
        .into_iter()
        .find(|(domain, _)| host == *domain || host.ends_with(&format!(".{domain}")))
        .map(|(_, provider)| provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nextcloud_dav_path_is_detected_as_nextcloud() {
        let provider = CaldavProvider::detected(
            "https://cloud.example.org",
            ["/remote.php/dav/calendars/user/personal/"],
        );
        assert!(provider.nextcloud_web_root().is_some());
    }

    #[test]
    fn an_icloud_shard_host_is_detected_as_icloud() {
        assert_eq!(
            CaldavProvider::detected("https://p42-caldav.icloud.com", ["/123456/calendars/home/"]),
            CaldavProvider::Icloud
        );
    }

    #[test]
    fn google_caldav_api_host_is_detected_as_google_calendar() {
        assert_eq!(
            CaldavProvider::detected(
                "https://apidata.googleusercontent.com/caldav/v2/",
                ["/caldav/v2/user%40gmail.com/events/"]
            ),
            CaldavProvider::GoogleCalendar
        );
    }

    #[test]
    fn a_lookalike_domain_is_not_a_hosted_provider() {
        assert_eq!(
            CaldavProvider::detected("https://notmailbox.org", ["/dav/calendars/user/home/"]),
            CaldavProvider::Unrecognised
        );
    }
}
