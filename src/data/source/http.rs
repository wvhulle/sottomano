use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

use reqwest::{
    Method,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Header(String),
    Client(String),
    Request(String),
    Body(String),
    Empty,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Header(name) => write!(f, "invalid header {name}"),
            Error::Client(detail) => write!(f, "client setup failed: {detail}"),
            Error::Request(detail) => write!(f, "{detail}"),
            Error::Body(detail) => write!(f, "unreadable response: {detail}"),
            Error::Empty => write!(f, "empty response"),
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
    pub body: Option<Value>,
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
    let body = client(timeout_secs)?.get(url).send().await?.text().await?;
    if body.trim().is_empty() {
        return Err(Error::Empty);
    }
    Ok(body)
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
    let bytes = resp.bytes().await?;
    Ok(Response {
        status,
        body: serde_json::from_slice(&bytes).ok(),
    })
}
