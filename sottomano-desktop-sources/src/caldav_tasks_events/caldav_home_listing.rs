use std::{borrow::Cow, collections::HashSet};

use fast_dav_rs::{Depth, Operation};
use roxmltree::{Document, ExpandedName, Node};

use crate::caldav_tasks_events::{
    caldav_agenda::Error,
    caldav_calendar_discovery::{CalendarHome, last_path_segment},
};

const DAV_NAMESPACE: &str = "DAV:";
const CALENDARSERVER_NAMESPACE: &str = "http://calendarserver.org/ns/";
const OWNCLOUD_NAMESPACE: &str = "http://owncloud.org/ns";
const HIDDEN_IN_NEXTCLOUD: &str = "0";
const LISTING_PROPFIND: &str = r#"
<d:propfind xmlns:d="DAV:" xmlns:cs="http://calendarserver.org/ns/" xmlns:oc="http://owncloud.org/ns">
  <d:prop>
    <d:displayname/>
    <d:resourcetype/>
    <cs:source/>
    <oc:calendar-enabled/>
  </d:prop>
</d:propfind>
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribedCalendar {
    pub href: String,
    pub name: String,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarHomeListing {
    pub hidden_calendar_hrefs: HashSet<String>,
    pub subscribed_calendars: Vec<SubscribedCalendar>,
}

impl CalendarHomeListing {
    #[must_use]
    pub fn visibility_of(&self, calendar_href: &str) -> CalendarVisibility {
        if self.hidden_calendar_hrefs.contains(calendar_href) {
            CalendarVisibility::Hidden
        } else {
            CalendarVisibility::Shown
        }
    }
}

impl CalendarHome {
    pub async fn home_listing(&self) -> Result<CalendarHomeListing, Error> {
        let response = self
            .client
            .propfind(&self.path, Depth::One, LISTING_PROPFIND)
            .await?;
        if !response.status().is_success() {
            return Err(fast_dav_rs::Error::unexpected_status(
                Operation::PropfindCollections,
                response.status(),
            )
            .into());
        }
        Ok(parsed_home_listing(&String::from_utf8_lossy(
            response.body(),
        ))?)
    }
}

pub fn parsed_home_listing(multistatus: &str) -> Result<CalendarHomeListing, roxmltree::Error> {
    let document = Document::parse(multistatus)?;
    let responses: Vec<Node> = document
        .descendants()
        .filter(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "response")))
        .collect();
    Ok(CalendarHomeListing {
        hidden_calendar_hrefs: responses
            .iter()
            .copied()
            .filter(|response| nextcloud_visibility(*response) == CalendarVisibility::Hidden)
            .filter_map(response_href)
            .map(str::to_owned)
            .collect(),
        subscribed_calendars: responses
            .into_iter()
            .filter(|response| nextcloud_visibility(*response) == CalendarVisibility::Shown)
            .filter_map(subscribed_calendar)
            .collect(),
    })
}

fn subscribed_calendar(response: Node) -> Option<SubscribedCalendar> {
    if !response
        .descendants()
        .any(|node| node.tag_name() == ExpandedName::from((CALENDARSERVER_NAMESPACE, "subscribed")))
    {
        return None;
    }
    let href = response_href(response)?;
    let source = response
        .descendants()
        .find(|node| node.tag_name() == ExpandedName::from((CALENDARSERVER_NAMESPACE, "source")))?;
    let source_url = source
        .children()
        .find(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "href")))?
        .text()?;
    let display_name = response
        .descendants()
        .find(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "displayname")))
        .and_then(|node| node.text())
        .map(str::trim)
        .filter(|name| !name.is_empty());
    Some(SubscribedCalendar {
        href: href.to_owned(),
        name: display_name.map_or_else(|| last_path_segment(href), str::to_owned),
        source_url: fetchable_feed_url(source_url.trim()).into_owned(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarVisibility {
    Shown,
    Hidden,
}

fn nextcloud_visibility(response: Node) -> CalendarVisibility {
    if response
        .descendants()
        .find(|node| {
            node.tag_name() == ExpandedName::from((OWNCLOUD_NAMESPACE, "calendar-enabled"))
        })
        .and_then(|node| node.text())
        .is_some_and(|enabled| enabled.trim() == HIDDEN_IN_NEXTCLOUD)
    {
        CalendarVisibility::Hidden
    } else {
        CalendarVisibility::Shown
    }
}

fn response_href<'document>(response: Node<'document, '_>) -> Option<&'document str> {
    response
        .children()
        .find(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "href")))?
        .text()
}

fn fetchable_feed_url(source_url: &str) -> Cow<'_, str> {
    ["webcal://", "webcals://"]
        .into_iter()
        .find_map(|scheme| source_url.strip_prefix(scheme))
        .map_or(Cow::Borrowed(source_url), |rest| {
            Cow::Owned(format!("https://{rest}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing() -> CalendarHomeListing {
        parsed_home_listing(include_str!("caldav_fixtures/nextcloud_home_listing.xml")).unwrap()
    }

    #[test]
    fn a_calendar_hidden_in_the_nextcloud_ui_is_listed_as_hidden() {
        let listing = listing();
        assert_eq!(
            listing.hidden_calendar_hrefs,
            HashSet::from([
                "/remote.php/dav/calendars/demo/archive/".to_owned(),
                "/remote.php/dav/calendars/demo/holidays/".to_owned(),
            ])
        );
        assert_eq!(
            listing.visibility_of("/remote.php/dav/calendars/demo/personal/"),
            CalendarVisibility::Shown
        );
    }

    #[test]
    fn a_visible_subscription_keeps_its_name_and_https_source() {
        assert_eq!(
            listing().subscribed_calendars,
            [SubscribedCalendar {
                href: "/remote.php/dav/calendars/demo/work-outlook/".to_owned(),
                name: "Work".to_owned(),
                source_url: "https://outlook.example.com/owa/calendar/reachcalendar.ics?a=1&b=2"
                    .to_owned(),
            }]
        );
    }

    #[test]
    fn a_hidden_subscription_is_not_downloaded() {
        assert!(
            listing()
                .subscribed_calendars
                .iter()
                .all(|subscription| !subscription.href.ends_with("/holidays/"))
        );
    }
}
