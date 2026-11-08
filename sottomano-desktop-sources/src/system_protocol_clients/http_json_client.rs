use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult},
    iter::successors,
    net::SocketAddr,
    path::Path,
    time::Duration,
};

use reqwest::{
    ClientBuilder, Method, Url,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::Value;
use tokio::{net::lookup_host, time::timeout};
use uom::si::{f64::Ratio, ratio::percent};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(5);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const TOO_MANY_REQUESTS_STATUS: u16 = 429;
pub const SERVER_ERROR_STATUS: u16 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Url(String),
    Header(String),
    Client(String),
    HostNotResolved {
        host: String,
    },
    TimedOut {
        host: String,
        after: Duration,
    },
    Unreachable {
        host: String,
        detail: String,
    },
    Body(String),
    Empty,
    Status {
        status: u16,
        message: Option<String>,
    },
    Decode(String),
}

impl Error {
    #[must_use]
    pub fn status(status: u16, body: Option<Value>) -> Self {
        Error::Status {
            status,
            message: failure_message(body),
        }
    }
}

pub fn failure_message(body: Option<Value>) -> Option<String> {
    decode::<FailureExplanation>(body).ok().map(String::from)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FailureExplanation {
    Wrapped {
        error: Box<FailureExplanation>,
    },
    Stated {
        #[serde(alias = "reason")]
        message: String,
    },
}

impl From<FailureExplanation> for String {
    fn from(explanation: FailureExplanation) -> Self {
        match explanation {
            FailureExplanation::Wrapped { error } => String::from(*error),
            FailureExplanation::Stated { message } => message,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Url(url) => write!(f, "invalid address {url}"),
            Error::Header(name) => write!(f, "invalid header {name}"),
            Error::Client(detail) => write!(f, "client setup failed: {detail}"),
            Error::HostNotResolved { host } => write!(f, "{host} could not be resolved"),
            Error::TimedOut { host, after } => {
                write!(f, "{host} did not answer within {} s", after.as_secs())
            }
            Error::Unreachable { host, detail } => write!(f, "{host} is unreachable: {detail}"),
            Error::Body(detail) => write!(f, "unreadable response: {detail}"),
            Error::Empty => write!(f, "empty response"),
            Error::Status {
                status,
                message: Some(message),
            } => write!(f, "{message} (HTTP {status})"),
            Error::Status {
                status,
                message: None,
            } => write!(f, "HTTP {status}"),
            Error::Decode(detail) => write!(f, "unexpected response: {detail}"),
        }
    }
}

impl StdError for Error {}

pub fn deserialize_percent_ratio<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Ratio, D::Error> {
    f64::deserialize(deserializer).map(Ratio::new::<percent>)
}

pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Option<Value>,
}

impl Response {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }
}

pub async fn get(url: &str, headers: &[(&str, &str)]) -> Result<Response, Error> {
    request(&Method::GET, url, headers, None).await
}

pub async fn post_json(
    url: &str,
    headers: &[(&str, &str)],
    body: &Value,
) -> Result<Response, Error> {
    request(&Method::POST, url, headers, Some(body)).await
}

pub async fn get_text(url: &str) -> Result<String, Error> {
    let fetched = fetched(&Method::GET, url, HeaderMap::new(), None).await?;
    let body =
        String::from_utf8(fetched.body_bytes).map_err(|error| Error::Body(error.to_string()))?;
    if !(200..300).contains(&fetched.status) {
        return Err(Error::status(
            fetched.status,
            serde_json::from_str(&body).ok(),
        ));
    }
    if body.trim().is_empty() {
        return Err(Error::Empty);
    }
    Ok(body)
}

pub async fn get_json<T: DeserializeOwned>(
    url: &str,
    headers: &[(&str, &str)],
) -> Result<T, Error> {
    let response = get(url, headers).await?;
    if response.status != 200 {
        return Err(Error::status(response.status, response.body));
    }
    decode(response.body)
}

struct UnixSocketReply {
    status: u16,
    body: Option<Value>,
}

async fn exchanged_over_unix_socket(
    socket_path: &Path,
    method: Method,
    url: &str,
) -> Result<UnixSocketReply, Error> {
    let socket_label = socket_path.display().to_string();
    let client = ClientBuilder::new()
        .connect_timeout(CONNECT_TIMEOUT)
        .unix_socket(socket_path)
        .build()
        .map_err(|error| Error::Client(error.to_string()))?;
    let answered = async {
        let response = client
            .request(method, url)
            .send()
            .await
            .map_err(|failure| Error::Unreachable {
                host: socket_label.clone(),
                detail: innermost_cause(&failure),
            })?;
        let status = response.status().as_u16();
        let body_bytes = response
            .bytes()
            .await
            .map_err(|error| Error::Body(innermost_cause(&error)))?;
        Ok(UnixSocketReply {
            status,
            body: serde_json::from_slice(&body_bytes).ok(),
        })
    };
    timeout(REQUEST_TIMEOUT, answered)
        .await
        .map_err(|_| Error::TimedOut {
            host: socket_label.clone(),
            after: REQUEST_TIMEOUT,
        })?
}

pub async fn get_json_over_unix_socket<T: DeserializeOwned>(
    socket_path: &Path,
    url: &str,
) -> Result<T, Error> {
    let reply = exchanged_over_unix_socket(socket_path, Method::GET, url).await?;
    if reply.status != 200 {
        return Err(Error::status(reply.status, reply.body));
    }
    decode(reply.body)
}

pub async fn post_over_unix_socket(socket_path: &Path, url: &str) -> Result<u16, Error> {
    let reply = exchanged_over_unix_socket(socket_path, Method::POST, url).await?;
    if reply.status >= 400 {
        return Err(Error::status(reply.status, reply.body));
    }
    Ok(reply.status)
}

pub fn decode<T: DeserializeOwned>(body: Option<Value>) -> Result<T, Error> {
    serde_json::from_value(body.unwrap_or(Value::Null))
        .map_err(|error| Error::Decode(error.to_string()))
}

struct ResolvedTarget {
    url: Url,
    host: String,
    pinned_addresses: PinnedAddresses,
}

enum PinnedAddresses {
    Resolved {
        domain: String,
        addresses: Vec<SocketAddr>,
    },
    LiteralAddress,
}

struct FetchedBytes {
    status: u16,
    headers: HeaderMap,
    body_bytes: Vec<u8>,
}

async fn resolved_target(url: &str) -> Result<ResolvedTarget, Error> {
    let url = Url::parse(url).map_err(|_| Error::Url(url.to_owned()))?;
    let host = url
        .host_str()
        .ok_or_else(|| Error::Url(url.to_string()))?
        .to_owned();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| Error::Url(url.to_string()))?;
    let pinned_addresses = match url.domain() {
        Some(domain) => PinnedAddresses::Resolved {
            domain: domain.to_owned(),
            addresses: resolved_addresses(domain, port).await?,
        },
        None => PinnedAddresses::LiteralAddress,
    };
    Ok(ResolvedTarget {
        url,
        host,
        pinned_addresses,
    })
}

async fn resolved_addresses(domain: &str, port: u16) -> Result<Vec<SocketAddr>, Error> {
    let unresolved = || Error::HostNotResolved {
        host: domain.to_owned(),
    };
    let addresses: Vec<SocketAddr> = timeout(RESOLVE_TIMEOUT, lookup_host((domain, port)))
        .await
        .map_err(|_| unresolved())?
        .map_err(|_| unresolved())?
        .collect();
    if addresses.is_empty() {
        return Err(unresolved());
    }
    Ok(addresses)
}

impl ResolvedTarget {
    fn client(&self) -> Result<reqwest::Client, Error> {
        let builder = ClientBuilder::new().connect_timeout(CONNECT_TIMEOUT);
        let builder = match &self.pinned_addresses {
            PinnedAddresses::Resolved { domain, addresses } => {
                builder.resolve_to_addrs(domain, addresses)
            }
            PinnedAddresses::LiteralAddress => builder,
        };
        builder
            .build()
            .map_err(|error| Error::Client(error.to_string()))
    }

    fn unreachable(&self, failure: &reqwest::Error) -> Error {
        Error::Unreachable {
            host: self.host.clone(),
            detail: innermost_cause(failure),
        }
    }
}

fn innermost_cause(failure: &(dyn StdError + 'static)) -> String {
    successors(Some(failure), |&cause| cause.source())
        .last()
        .map_or_else(|| failure.to_string(), ToString::to_string)
}

fn header_map(headers: &[(&str, &str)]) -> Result<HeaderMap, Error> {
    let mut header_map = HeaderMap::with_capacity(headers.len() + 1);
    for (name, value) in headers {
        header_map.insert(
            HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| Error::Header((*name).to_owned()))?,
            HeaderValue::from_str(value).map_err(|_| Error::Header((*name).to_owned()))?,
        );
    }
    Ok(header_map)
}

async fn fetched(
    method: &Method,
    url: &str,
    header_map: HeaderMap,
    json_body: Option<&Value>,
) -> Result<FetchedBytes, Error> {
    let target = resolved_target(url).await?;
    let mut request_builder = target
        .client()?
        .request(method.clone(), target.url.clone())
        .headers(header_map);
    if let Some(body) = json_body {
        request_builder = request_builder
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(body).map_err(|error| Error::Body(error.to_string()))?);
    }
    let answered = async {
        let response = request_builder
            .send()
            .await
            .map_err(|failure| target.unreachable(&failure))?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body_bytes = response
            .bytes()
            .await
            .map_err(|error| Error::Body(innermost_cause(&error)))?
            .to_vec();
        Ok(FetchedBytes {
            status,
            headers,
            body_bytes,
        })
    };
    timeout(REQUEST_TIMEOUT, answered)
        .await
        .map_err(|_| Error::TimedOut {
            host: target.host.clone(),
            after: REQUEST_TIMEOUT,
        })?
}

async fn request(
    method: &Method,
    url: &str,
    headers: &[(&str, &str)],
    json_body: Option<&Value>,
) -> Result<Response, Error> {
    let fetched = fetched(method, url, header_map(headers)?, json_body).await?;
    Ok(Response {
        status: fetched.status,
        headers: fetched.headers,
        body: serde_json::from_slice(&fetched.body_bytes).ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unparsable_body_falls_back_to_the_status() {
        let failure = Error::status(503, Some(Value::Null));
        assert_eq!(
            failure,
            Error::Status {
                status: 503,
                message: None
            }
        );
        assert_eq!(failure.to_string(), "HTTP 503");
    }

    #[test]
    fn an_api_message_is_shown_beside_the_status() {
        let body = serde_json::json!({"error": {"message": "usage limit reached"}});
        let failure = Error::status(429, Some(body));
        assert_eq!(failure.to_string(), "usage limit reached (HTTP 429)");
    }

    #[tokio::test]
    async fn a_reserved_invalid_domain_fails_at_resolution() {
        assert_eq!(
            get_text("http://sottomano-check.invalid/").await,
            Err(Error::HostNotResolved {
                host: "sottomano-check.invalid".to_owned()
            })
        );
    }

    #[tokio::test]
    async fn a_closed_local_port_fails_at_connection() {
        assert!(matches!(
            get_text("http://127.0.0.1:9/").await,
            Err(Error::Unreachable { host, .. }) if host == "127.0.0.1"
        ));
    }
}
