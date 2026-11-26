mod pinned_address_fetches;

use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult},
    iter::successors,
    path::Path,
    time::Duration,
};

use pinned_address_fetches::{fetched, header_map};
use reqwest::{
    ClientBuilder, Method,
    header::{CONTENT_TYPE, HeaderMap},
};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::Value;
use tokio::time::timeout;
use uom::si::{f64::Ratio, ratio::percent};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
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
        message: StatusExplanation,
    },
    Decode(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusExplanation {
    Stated(String),
    Unexplained,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseBody {
    Json(Value),
    NotJson,
}

impl Error {
    #[must_use]
    pub fn status(status: u16, body: ResponseBody) -> Self {
        Error::Status {
            status,
            message: failure_message(body),
        }
    }
}

#[must_use]
pub fn failure_message(body: ResponseBody) -> StatusExplanation {
    decode::<FailureExplanation>(body)
        .inspect_err(|decode_failure| {
            tracing::debug!("http failure body carries no explanation: {decode_failure}");
        })
        .map_or(StatusExplanation::Unexplained, |explanation| {
            StatusExplanation::Stated(String::from(explanation))
        })
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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Url(url) => write!(formatter, "invalid address {url}"),
            Error::Header(name) => write!(formatter, "invalid header {name}"),
            Error::Client(detail) => write!(formatter, "client setup failed: {detail}"),
            Error::HostNotResolved { host } => write!(formatter, "{host} could not be resolved"),
            Error::TimedOut { host, after } => {
                write!(
                    formatter,
                    "{host} did not answer within {} s",
                    after.as_secs()
                )
            }
            Error::Unreachable { host, detail } => {
                write!(formatter, "{host} is unreachable: {detail}")
            }
            Error::Body(detail) => write!(formatter, "unreadable response: {detail}"),
            Error::Empty => write!(formatter, "empty response"),
            Error::Status {
                status,
                message: StatusExplanation::Stated(message),
            } => write!(formatter, "{message} (HTTP {status})"),
            Error::Status {
                status,
                message: StatusExplanation::Unexplained,
            } => write!(formatter, "HTTP {status}"),
            Error::Decode(detail) => write!(formatter, "unexpected response: {detail}"),
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
    pub body: ResponseBody,
}

impl Response {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| {
            value
                .to_str()
                .inspect_err(|text_failure| {
                    tracing::warn!("http header {name} is not visible ascii: {text_failure}");
                })
                .ok()
        })
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
            serde_json::from_str(&body)
                .inspect_err(|parse_failure| {
                    tracing::debug!(
                        "http {url}: failure body {} is not json: {parse_failure}",
                        fetched.status
                    );
                })
                .map_or(ResponseBody::NotJson, ResponseBody::Json),
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
    body: ResponseBody,
}

async fn exchanged_over_unix_socket(
    socket_path: &Path,
    method: Method,
    url: &str,
    json_body: Option<&Value>,
) -> Result<UnixSocketReply, Error> {
    let socket_label = socket_path.display().to_string();
    let client = ClientBuilder::new()
        .connect_timeout(CONNECT_TIMEOUT)
        .unix_socket(socket_path)
        .build()
        .map_err(|error| Error::Client(error.to_string()))?;
    let answered = async {
        let request = client.request(method, url);
        let request = match json_body {
            Some(json_body) => request.header(CONTENT_TYPE, "application/json").body(
                serde_json::to_vec(json_body).map_err(|error| Error::Body(error.to_string()))?,
            ),
            None => request,
        };
        let response = request.send().await.map_err(|failure| Error::Unreachable {
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
            body: serde_json::from_slice(&body_bytes)
                .inspect_err(|parse_failure| {
                    tracing::debug!(
                        "http {url} over {socket_label}: body is not json: {parse_failure}"
                    );
                })
                .map_or(ResponseBody::NotJson, ResponseBody::Json),
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
    let reply = exchanged_over_unix_socket(socket_path, Method::GET, url, None).await?;
    if reply.status != 200 {
        return Err(Error::status(reply.status, reply.body));
    }
    decode(reply.body)
}

pub async fn post_over_unix_socket(socket_path: &Path, url: &str) -> Result<u16, Error> {
    let reply = exchanged_over_unix_socket(socket_path, Method::POST, url, None).await?;
    if reply.status >= 400 {
        return Err(Error::status(reply.status, reply.body));
    }
    Ok(reply.status)
}

pub async fn put_over_unix_socket(
    socket_path: &Path,
    url: &str,
    json_body: Option<&Value>,
) -> Result<u16, Error> {
    let reply = exchanged_over_unix_socket(socket_path, Method::PUT, url, json_body).await?;
    if reply.status >= 400 {
        return Err(Error::status(reply.status, reply.body));
    }
    Ok(reply.status)
}

pub fn decode<T: DeserializeOwned>(body: ResponseBody) -> Result<T, Error> {
    let json_value = match body {
        ResponseBody::Json(json_value) => json_value,
        ResponseBody::NotJson => Value::Null,
    };
    serde_json::from_value(json_value).map_err(|error| Error::Decode(error.to_string()))
}

fn innermost_cause(failure: &(dyn StdError + 'static)) -> String {
    successors(Some(failure), |&cause| cause.source())
        .last()
        .map_or_else(|| failure.to_string(), ToString::to_string)
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
        body: serde_json::from_slice(&fetched.body_bytes)
            .inspect_err(|parse_failure| {
                tracing::debug!("http {url}: body is not json: {parse_failure}");
            })
            .map_or(ResponseBody::NotJson, ResponseBody::Json),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unparsable_body_falls_back_to_the_status() {
        let failure = Error::status(503, ResponseBody::Json(Value::Null));
        assert_eq!(
            failure,
            Error::Status {
                status: 503,
                message: StatusExplanation::Unexplained
            }
        );
        assert_eq!(failure.to_string(), "HTTP 503");
    }

    #[test]
    fn an_api_message_is_shown_beside_the_status() {
        let body = serde_json::json!({"error": {"message": "usage limit reached"}});
        let failure = Error::status(429, ResponseBody::Json(body));
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
