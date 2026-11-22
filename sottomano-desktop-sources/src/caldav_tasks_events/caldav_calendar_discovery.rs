use http::Uri;
use libdav::{
    CalDavClient, Depth,
    caldav::FindCalendarHomeSet,
    caldav_service_for_url,
    dav::{FindCurrentUserPrincipalError, ResolveContextPathError, WebDavClient},
};
use roxmltree::Document;
use tokio::time::timeout;

use crate::{
    StateChange,
    caldav_tasks_events::{
        caldav_agenda::{CalendarHolding, Error},
        caldav_webdav_requests::{
            CALDAV_NAMESPACE, CaldavHttpClient, DAV_NAMESPACE, DavMethod, MultistatusRequest,
            authorized_http_client, dav_responses, found_property, found_property_text,
            response_href,
        },
    },
    system_protocol_clients::http_json_client::REQUEST_TIMEOUT,
};

const CALENDARSERVER_NAMESPACE: &str = "http://calendarserver.org/ns/";
const UNAUTHORIZED_STATUS: u16 = 401;
const CALENDAR_LISTING_PROPFIND: &str =
    include_str!("caldav_request_bodies/calendar_listing_propfind.xml");

#[derive(Debug, Clone, Copy)]
pub struct CaldavLogin<'credentials> {
    pub server_url: &'credentials str,
    pub user: &'credentials str,
    pub password: &'credentials str,
}

#[derive(Clone)]
pub struct CalendarHome {
    pub client: CalDavClient<CaldavHttpClient>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeTracking {
    Token(String),
    Untracked,
}

impl ChangeTracking {
    #[must_use]
    pub fn change_since(&self, earlier: &Self) -> StateChange {
        match (self, earlier) {
            (ChangeTracking::Token(current), ChangeTracking::Token(previous))
                if current == previous =>
            {
                StateChange::Unchanged
            }
            _ => StateChange::Changed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarCollection {
    pub href: String,
    pub name: String,
    pub event_holding: CalendarHolding,
    pub task_holding: CalendarHolding,
    pub change_tracking: ChangeTracking,
}

pub fn parsed_calendar_collections(
    multistatus: &str,
) -> Result<Vec<CalendarCollection>, roxmltree::Error> {
    let document = Document::parse(multistatus)?;
    Ok(dav_responses(&document)
        .filter(|response| {
            found_property(*response, DAV_NAMESPACE, "resourcetype").is_some_and(|resource_type| {
                resource_type
                    .children()
                    .any(|node| node.has_tag_name((CALDAV_NAMESPACE, "calendar")))
            })
        })
        .filter_map(|response| {
            let href = response_href(response)?;
            let supported_components: Vec<&str> = found_property(
                response,
                CALDAV_NAMESPACE,
                "supported-calendar-component-set",
            )
            .map(|component_set| {
                component_set
                    .children()
                    .filter(|node| node.has_tag_name((CALDAV_NAMESPACE, "comp")))
                    .filter_map(|component| component.attribute("name"))
                    .collect()
            })
            .unwrap_or_default();
            let advertised_holding = |component: &str| {
                if supported_components.is_empty()
                    || supported_components
                        .iter()
                        .any(|supported| supported.eq_ignore_ascii_case(component))
                {
                    CalendarHolding::Holds
                } else {
                    CalendarHolding::Lacks
                }
            };
            Some(CalendarCollection {
                event_holding: advertised_holding("VEVENT"),
                task_holding: advertised_holding("VTODO"),
                name: found_property_text(response, DAV_NAMESPACE, "displayname")
                    .map_or_else(|| last_path_segment(href), str::to_owned),
                change_tracking: found_property_text(response, DAV_NAMESPACE, "sync-token")
                    .or_else(|| found_property_text(response, CALENDARSERVER_NAMESPACE, "getctag"))
                    .or_else(|| found_property_text(response, DAV_NAMESPACE, "getetag"))
                    .map_or(ChangeTracking::Untracked, |token| {
                        ChangeTracking::Token(token.to_owned())
                    }),
                href: href.to_owned(),
            })
        })
        .collect())
}

pub(super) async fn within_request_timeout<Output>(
    request: impl Future<Output = Result<Output, Error>>,
) -> Result<Output, Error> {
    timeout(REQUEST_TIMEOUT, request)
        .await
        .map_err(|_| Error::TimedOut)?
}

pub async fn connect_calendar_home(login: CaldavLogin<'_>) -> Result<CalendarHome, Error> {
    let server_url: Uri = login.server_url.parse()?;
    let service = caldav_service_for_url(&server_url)?;
    let host = server_url
        .host()
        .ok_or(Error::ServerUrlWithoutHost)?
        .to_owned();
    let port = server_url
        .port_u16()
        .unwrap_or_else(|| service.default_port());
    let http_client = authorized_http_client(login.user, login.password, REQUEST_TIMEOUT);
    let context_probe = WebDavClient::new(server_url.clone(), http_client.clone());
    let context_url = within_request_timeout(async {
        context_probe
            .find_context_path(service, &host, port)
            .await
            .map_err(|failure| match failure {
                ResolveContextPathError::BadStatusCode(status)
                    if status.as_u16() == UNAUTHORIZED_STATUS =>
                {
                    Error::Rejected
                }
                other => Error::ContextPathUnresolved(other),
            })
    })
    .await?;
    let client = CalDavClient::new(WebDavClient::new(
        context_url.unwrap_or(server_url),
        http_client,
    ));
    let principal = within_request_timeout(async {
        client
            .find_current_user_principal()
            .await
            .map_err(|failure| match failure {
                FindCurrentUserPrincipalError::RequestError(request_failure) => {
                    Error::from(request_failure)
                }
                invalid_input @ FindCurrentUserPrincipalError::InvalidInput(_) => {
                    Error::PrincipalLookup(invalid_input)
                }
            })
    })
    .await?
    .ok_or(Error::NoCurrentUserPrincipal)?;
    let home_set = within_request_timeout(async {
        Ok(client
            .request(FindCalendarHomeSet::new(principal.path()))
            .await?)
    })
    .await?
    .home_sets
    .into_iter()
    .next()
    .ok_or(Error::NoCalendarHomeSet)?;
    Ok(CalendarHome {
        client,
        path: home_set.path().to_owned(),
    })
}

impl CalendarHome {
    pub(super) async fn multistatus_request<Parsed>(
        &self,
        request: MultistatusRequest<'_, Parsed>,
    ) -> Result<Parsed, Error> {
        within_request_timeout(async { Ok(self.client.request(request).await?) }).await
    }

    pub async fn list_calendars(&self) -> Result<Vec<CalendarCollection>, Error> {
        self.multistatus_request(MultistatusRequest {
            method: DavMethod::Propfind,
            path: &self.path,
            depth: Depth::One,
            xml_body: CALENDAR_LISTING_PROPFIND.to_owned(),
            parse_multistatus: parsed_calendar_collections,
        })
        .await
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
    use super::*;

    fn listed_calendars() -> Vec<CalendarCollection> {
        parsed_calendar_collections(include_str!(
            "caldav_fixtures/nextcloud_calendar_listing.xml"
        ))
        .unwrap()
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
        assert_eq!(personal.event_holding, CalendarHolding::Holds);
        assert_eq!(personal.task_holding, CalendarHolding::Lacks);
        assert_eq!(personal.href, "/remote.php/dav/calendars/wvhulle/personal/");
    }

    #[test]
    fn a_calendar_without_display_name_is_named_after_its_path() {
        let tasks = &listed_calendars()[1];
        assert_eq!(tasks.name, "tasks");
        assert_eq!(tasks.task_holding, CalendarHolding::Holds);
        assert_eq!(tasks.event_holding, CalendarHolding::Lacks);
    }

    #[test]
    fn an_untracked_calendar_never_counts_as_unchanged() {
        assert_eq!(
            ChangeTracking::Untracked.change_since(&ChangeTracking::Untracked),
            StateChange::Changed
        );
        let token = ChangeTracking::Token("42".to_owned());
        assert_eq!(token.change_since(&token.clone()), StateChange::Unchanged);
    }
}
