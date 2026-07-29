//! In-process HTTP for the AI-usage sources — the faithful equivalent of odin's
//! nushell `http get`/`http post`. Unlike the curl-backed
//! [`command::http_get`](crate::data_sources::command::http_get), it sets an
//! `Authorization` header and does POST without ever putting the bearer token
//! on a process argv (visible in `ps`).
//!
//! `reqwest` is built with `default-features = false`, so the `json` feature is
//! off — bodies are (de)serialized with `serde_json` over the raw bytes rather
//! than via `RequestBuilder::json`/`Response::json`.
//!
//! The provider sources call these from their synchronous `poll`, which the app
//! runs on a `spawn_blocking` thread; the async request is driven there via
//! [`block_on`], reusing the app's runtime when one is in scope and otherwise
//! spinning up a throwaway current-thread one. `reqwest` sends no default
//! `User-Agent`, and GitHub's API rejects UA-less requests — so every caller
//! passes its full header set (see the per-provider modules).

use std::{future::Future, time::Duration};

use reqwest::{
    Method,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde_json::Value;
use tokio::runtime::{Builder, Handle};

/// A completed HTTP exchange: the status code and the JSON body (`None` when
/// the body was absent or not JSON). Callers inspect `status` themselves —
/// odin's scripts branch on it (e.g. Claude's 429 backoff), so this preserves
/// it rather than collapsing non-200 to a bare `None`.
pub struct Response {
    pub status: u16,
    pub body: Option<Value>,
}

pub fn get(url: &str, headers: &[(&str, &str)], timeout_secs: u64) -> Option<Response> {
    request(&Method::GET, url, headers, None, timeout_secs)
}

pub fn post_json(
    url: &str,
    headers: &[(&str, &str)],
    body: &Value,
    timeout_secs: u64,
) -> Option<Response> {
    request(&Method::POST, url, headers, Some(body), timeout_secs)
}

fn request(
    method: &Method,
    url: &str,
    headers: &[(&str, &str)],
    json_body: Option<&Value>,
    timeout_secs: u64,
) -> Option<Response> {
    let mut header_map = HeaderMap::with_capacity(headers.len() + 1);
    for (name, value) in headers {
        header_map.insert(
            HeaderName::from_bytes(name.as_bytes()).ok()?,
            HeaderValue::from_str(value).ok()?,
        );
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .ok()?;
    let mut req = client.request(method.clone(), url).headers(header_map);
    if let Some(body) = json_body {
        req = req
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(body).ok()?);
    }
    block_on(async move {
        let resp = req.send().await.ok()?;
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await.ok()?;
        Some(Response {
            status,
            body: serde_json::from_slice(&bytes).ok(),
        })
    })
}

fn block_on<F: Future>(future: F) -> F::Output {
    match Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build fallback tokio runtime")
            .block_on(future),
    }
}
