use fast_dav_rs::{CalDavClient, CalendarInfo, WebDavClient, discover_caldav};

use crate::{
    caldav_tasks_events::caldav_agenda::Error,
    system_protocol_clients::http_json_client::REQUEST_TIMEOUT,
};

#[derive(Debug, Clone, Copy)]
pub struct CaldavLogin<'a> {
    pub server_url: &'a str,
    pub user: &'a str,
    pub password: &'a str,
}

#[derive(Clone)]
pub struct CalendarHome {
    pub client: CalDavClient,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeTracking {
    Token(String),
    Untracked,
}

impl ChangeTracking {
    #[must_use]
    pub fn unchanged_since(&self, earlier: &Self) -> bool {
        matches!(
            (self, earlier),
            (ChangeTracking::Token(current), ChangeTracking::Token(previous)) if current == previous
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarCollection {
    pub href: String,
    pub name: String,
    pub holds_events: bool,
    pub holds_tasks: bool,
    pub change_tracking: ChangeTracking,
}

impl From<CalendarInfo> for CalendarCollection {
    fn from(info: CalendarInfo) -> Self {
        let advertises = |component: &str| {
            info.supported_components.is_empty()
                || info
                    .supported_components
                    .iter()
                    .any(|supported| supported.eq_ignore_ascii_case(component))
        };
        Self {
            holds_events: advertises("VEVENT"),
            holds_tasks: advertises("VTODO"),
            name: info
                .displayname
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map_or_else(|| last_path_segment(&info.href), str::to_owned),
            change_tracking: info
                .sync_token
                .or(info.etag)
                .map_or(ChangeTracking::Untracked, ChangeTracking::Token),
            href: info.href,
        }
    }
}

pub async fn connect_calendar_home(login: CaldavLogin<'_>) -> Result<CalendarHome, Error> {
    let probe = WebDavClient::builder(login.server_url)
        .basic_auth(login.user, login.password)
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let service_url = discover_caldav(&probe).await?;
    let client = CalDavClient::builder(service_url)
        .basic_auth(login.user, login.password)
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let Some(principal) = client.discover_current_user_principal().await? else {
        return Err(Error::NoCurrentUserPrincipal);
    };
    let Some(path) = client
        .discover_calendar_home_set(&principal)
        .await?
        .into_iter()
        .next()
    else {
        return Err(Error::NoCalendarHomeSet);
    };
    Ok(CalendarHome { client, path })
}

impl CalendarHome {
    pub async fn list_calendars(&self) -> Result<Vec<CalendarCollection>, Error> {
        Ok(self
            .client
            .list_calendars(&self.path)
            .await?
            .into_iter()
            .map(CalendarCollection::from)
            .collect())
    }
}

pub(super) fn last_path_segment(href: &str) -> String {
    href.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(href)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use fast_dav_rs::map_calendar_list;

    use super::*;

    fn listed_calendars() -> Vec<CalendarCollection> {
        let listing = include_bytes!("caldav_fixtures/nextcloud_calendar_listing.xml");
        let items = fast_dav_rs::parse_multistatus_bytes(listing).unwrap().items;
        map_calendar_list(items)
            .into_iter()
            .map(CalendarCollection::from)
            .collect()
    }

    #[test]
    fn the_calendar_home_itself_is_not_a_calendar() {
        assert_eq!(listed_calendars().len(), 2);
    }

    #[test]
    fn an_event_calendar_keeps_its_name_and_sync_token() {
        let personal = &listed_calendars()[0];
        assert_eq!(personal.name, "Personal");
        assert_eq!(
            personal.change_tracking,
            ChangeTracking::Token("http://sabre.io/ns/sync/42".to_owned())
        );
        assert!(personal.holds_events && !personal.holds_tasks);
        assert_eq!(personal.href, "/remote.php/dav/calendars/wvhulle/personal/");
    }

    #[test]
    fn a_calendar_without_display_name_is_named_after_its_path() {
        let tasks = &listed_calendars()[1];
        assert_eq!(tasks.name, "tasks");
        assert!(tasks.holds_tasks && !tasks.holds_events);
    }

    #[test]
    fn an_untracked_calendar_never_counts_as_unchanged() {
        assert!(!ChangeTracking::Untracked.unchanged_since(&ChangeTracking::Untracked));
        let token = ChangeTracking::Token("42".to_owned());
        assert!(token.unchanged_since(&token.clone()));
    }
}
