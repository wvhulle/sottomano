use std::time::Duration;

use http::{HeaderValue, Method, Request, Uri, response::Parts};
use hyper_tls::HttpsConnector;
use hyper_util::{
    client::legacy::{Client, Error as HyperClientFailure, connect::HttpConnector},
    rt::TokioExecutor,
};
use libdav::{
    Depth,
    dav::{WebDavError, make_relative_url},
    requests::{DavRequest, ParseResponseError, xml_content_type_header},
};
use roxmltree::{Document, ExpandedName, Node};
use tower_http::auth::AddAuthorization;

pub const DAV_NAMESPACE: &str = "DAV:";
pub const CALDAV_NAMESPACE: &str = "urn:ietf:params:xml:ns:caldav";
const SUCCESSFUL_PROPSTAT_CODE: &str = " 200 ";

pub type CaldavHttpClient = AddAuthorization<Client<HttpsConnector<HttpConnector>, String>>;
pub type CaldavClientFailure = HyperClientFailure;
pub type CaldavRequestError = WebDavError<CaldavClientFailure>;

#[must_use]
pub fn authorized_http_client(
    user: &str,
    password: &str,
    connect_timeout: Duration,
) -> CaldavHttpClient {
    let mut http_connector = HttpConnector::new();
    http_connector.enforce_http(false);
    http_connector.set_connect_timeout(Some(connect_timeout));
    let https_client = Client::builder(TokioExecutor::new())
        .build(HttpsConnector::new_with_connector(http_connector));
    AddAuthorization::basic(https_client, user, password)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DavMethod {
    Propfind,
    Report,
}

impl DavMethod {
    fn method_name(self) -> &'static [u8] {
        match self {
            DavMethod::Propfind => b"PROPFIND",
            DavMethod::Report => b"REPORT",
        }
    }
}

pub type MultistatusParser<Parsed> = fn(&str) -> Result<Parsed, roxmltree::Error>;

pub struct MultistatusRequest<'path, Parsed> {
    pub method: DavMethod,
    pub path: &'path str,
    pub depth: Depth,
    pub xml_body: String,
    pub parse_multistatus: MultistatusParser<Parsed>,
}

impl<Parsed> DavRequest for MultistatusRequest<'_, Parsed> {
    type Response = Parsed;
    type ParseError = ParseResponseError;
    type Error<E> = WebDavError<E>;

    fn prepare_request(&self, base_url: Uri) -> Result<Request<String>, http::Error> {
        let (content_type_name, content_type_value) = xml_content_type_header();
        Request::builder()
            .method(Method::from_bytes(self.method.method_name())?)
            .uri(make_relative_url(base_url, self.path)?)
            .header("Depth", HeaderValue::from(self.depth))
            .header(content_type_name, content_type_value)
            .body(self.xml_body.clone())
    }

    fn parse_response(&self, parts: &Parts, body: &[u8]) -> Result<Parsed, ParseResponseError> {
        if !parts.status.is_success() {
            return Err(ParseResponseError::BadStatusCode(parts.status));
        }
        Ok((self.parse_multistatus)(str::from_utf8(body)?)?)
    }
}

pub fn dav_responses<'document, 'input>(
    document: &'document Document<'input>,
) -> impl Iterator<Item = Node<'document, 'input>> {
    document
        .descendants()
        .filter(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "response")))
}

#[must_use]
pub fn response_href<'document>(response: Node<'document, '_>) -> Option<&'document str> {
    response
        .children()
        .find(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "href")))?
        .text()
        .map(str::trim)
}

#[must_use]
pub fn found_property<'document, 'input>(
    response: Node<'document, 'input>,
    namespace: &str,
    property_name: &str,
) -> Option<Node<'document, 'input>> {
    response
        .children()
        .filter(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "propstat")))
        .filter(|propstat| {
            propstat
                .children()
                .find(|node| node.tag_name() == ExpandedName::from((DAV_NAMESPACE, "status")))
                .and_then(|status| status.text())
                .is_some_and(|status_line| {
                    format!("{} ", status_line.trim()).contains(SUCCESSFUL_PROPSTAT_CODE)
                })
        })
        .find_map(|propstat| {
            propstat
                .descendants()
                .find(|node| node.tag_name() == ExpandedName::from((namespace, property_name)))
        })
}

#[must_use]
pub fn found_property_text<'document>(
    response: Node<'document, '_>,
    namespace: &str,
    property_name: &str,
) -> Option<&'document str> {
    found_property(response, namespace, property_name)?
        .text()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarObjectData {
    pub href: String,
    pub calendar_data: String,
}

pub fn parsed_calendar_objects(
    multistatus: &str,
) -> Result<Vec<CalendarObjectData>, roxmltree::Error> {
    let document = Document::parse(multistatus)?;
    Ok(dav_responses(&document)
        .filter_map(|response| {
            let href = response_href(response)?;
            let Some(calendar_data) = found_property(response, CALDAV_NAMESPACE, "calendar-data")
                .and_then(|calendar_data| calendar_data.text())
                .filter(|calendar_data| !calendar_data.trim().is_empty())
            else {
                tracing::warn!(object_href = href, "calendar object response has no data");
                return None;
            };
            Some(CalendarObjectData {
                href: href.to_owned(),
                calendar_data: calendar_data.to_owned(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_PROPSTATS: &str = r#"<d:multistatus xmlns:d="DAV:">
 <d:response>
  <d:href> /calendars/demo/ </d:href>
  <d:propstat>
   <d:prop><d:displayname>Demo</d:displayname></d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
  <d:propstat>
   <d:prop><d:getetag>"missing"</d:getetag></d:prop>
   <d:status>HTTP/1.1 404 Not Found</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

    #[test]
    fn a_property_is_read_only_from_a_successful_propstat() {
        let document = Document::parse(TWO_PROPSTATS).unwrap();
        let response = dav_responses(&document).next().unwrap();
        assert_eq!(response_href(response), Some("/calendars/demo/"));
        assert_eq!(
            found_property_text(response, DAV_NAMESPACE, "displayname"),
            Some("Demo")
        );
        assert_eq!(
            found_property_text(response, DAV_NAMESPACE, "getetag"),
            None
        );
    }

    #[test]
    fn a_report_keeps_only_objects_that_carry_calendar_data() {
        let objects =
            parsed_calendar_objects(include_str!("caldav_fixtures/calendar_query_report.xml"))
                .unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(
            objects[0].href,
            "/remote.php/dav/calendars/demo/personal/standup.ics"
        );
        assert!(objects[0].calendar_data.contains("SUMMARY:Standup"));
    }

    #[test]
    fn a_report_names_its_method_depth_and_xml_body() {
        let request = MultistatusRequest {
            method: DavMethod::Report,
            path: "/calendars/demo/personal/",
            depth: Depth::One,
            xml_body: "<C:calendar-query/>".to_owned(),
            parse_multistatus: |_| Ok(()),
        };
        let prepared = request
            .prepare_request(Uri::from_static("https://cloud.example.com"))
            .unwrap();
        assert_eq!(prepared.method().as_str(), "REPORT");
        assert_eq!(
            prepared.uri().to_string(),
            "https://cloud.example.com/calendars/demo/personal/"
        );
        assert_eq!(prepared.headers()["Depth"], "1");
        assert_eq!(prepared.body(), "<C:calendar-query/>");
    }
}
