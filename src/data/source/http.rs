use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

use reqwest::{
    Method,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use crate::widgets::tooltip_body::{KeyValue, Section};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Header(String),
    Client(String),
    Request(String),
    Body(String),
    Empty,
    Status {
        status: u16,
        message: Option<String>,
    },
    Decode(String),
}

impl Error {
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
            Error::Header(name) => write!(f, "invalid header {name}"),
            Error::Client(detail) => write!(f, "client setup failed: {detail}"),
            Error::Request(detail) => write!(f, "{detail}"),
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

impl From<Error> for Section {
    fn from(error: Error) -> Self {
        match error {
            Error::Status { status, message } => Section::dictionary(
                message
                    .map(|message| KeyValue::new("Error", message))
                    .into_iter()
                    .chain([KeyValue::new("Status", status.to_string())]),
            ),
            other => Section::error(other),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Error::Request("timed out".to_owned())
        } else if error.is_connect() {
            Error::Request("cannot reach host".to_owned())
        } else {
            Error::Request(error.to_string())
        }
    }
}

pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Option<Value>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }
}

pub async fn get(
    url: &str,
    headers: &[(&str, &str)],
    timeout_secs: u64,
) -> Result<Response, Error> {
    request(&Method::GET, url, headers, None, timeout_secs).await
}

pub async fn post_json(
    url: &str,
    headers: &[(&str, &str)],
    body: &Value,
    timeout_secs: u64,
) -> Result<Response, Error> {
    request(&Method::POST, url, headers, Some(body), timeout_secs).await
}

pub async fn get_text(url: &str, timeout_secs: u64) -> Result<String, Error> {
    let response = client(timeout_secs)?.get(url).send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(Error::status(
            status.as_u16(),
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
    timeout_secs: u64,
) -> Result<T, Error> {
    let response = get(url, headers, timeout_secs).await?;
    if response.status != 200 {
        return Err(Error::status(response.status, response.body));
    }
    decode(response.body)
}

pub fn decode<T: DeserializeOwned>(body: Option<Value>) -> Result<T, Error> {
    serde_json::from_value(body.unwrap_or(Value::Null))
        .map_err(|error| Error::Decode(error.to_string()))
}

fn client(timeout_secs: u64) -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|error| Error::Client(error.to_string()))
}

async fn request(
    method: &Method,
    url: &str,
    headers: &[(&str, &str)],
    json_body: Option<&Value>,
    timeout_secs: u64,
) -> Result<Response, Error> {
    let mut header_map = HeaderMap::with_capacity(headers.len() + 1);
    for (name, value) in headers {
        header_map.insert(
            HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| Error::Header((*name).to_owned()))?,
            HeaderValue::from_str(value).map_err(|_| Error::Header((*name).to_owned()))?,
        );
    }
    let mut req = client(timeout_secs)?
        .request(method.clone(), url)
        .headers(header_map);
    if let Some(body) = json_body {
        req = req
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(body).map_err(|error| Error::Body(error.to_string()))?);
    }
    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let bytes = resp.bytes().await?;
    Ok(Response {
        status,
        headers,
        body: serde_json::from_slice(&bytes).ok(),
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
}
