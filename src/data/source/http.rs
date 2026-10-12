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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Header(String),
    Client(String),
    Request(String),
    Body(String),
    Empty,
    Status(u16),
    Decode(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Header(name) => write!(f, "invalid header {name}"),
            Error::Client(detail) => write!(f, "client setup failed: {detail}"),
            Error::Request(detail) => write!(f, "{detail}"),
            Error::Body(detail) => write!(f, "unreadable response: {detail}"),
            Error::Empty => write!(f, "empty response"),
            Error::Status(code) => write!(f, "HTTP {code}"),
            Error::Decode(detail) => write!(f, "unexpected response: {detail}"),
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
    if !status.is_success() {
        return Err(Error::Status(status.as_u16()));
    }
    let body = response.text().await?;
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
        return Err(Error::Status(response.status));
    }
    decode(response.body)
}

pub fn decode<T: DeserializeOwned>(body: Option<Value>) -> Result<T, Error> {
    serde_json::from_value(body.unwrap_or(Value::Null))
        .map_err(|error| Error::Decode(error.to_string()))
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiMessage,
}

#[derive(Deserialize)]
struct ApiMessage {
    message: String,
}

pub fn api_message(body: Option<Value>) -> Option<String> {
    decode::<ApiError>(body).ok().map(|api| api.error.message)
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
