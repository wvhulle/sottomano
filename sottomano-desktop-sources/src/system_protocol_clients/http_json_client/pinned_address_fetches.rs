use std::{net::SocketAddr, time::Duration};

use reqwest::{
    ClientBuilder, Method, Url,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde_json::Value;
use tokio::{net::lookup_host, time::timeout};

use super::{CONNECT_TIMEOUT, Error, REQUEST_TIMEOUT, innermost_cause};

const RESOLVE_TIMEOUT: Duration = Duration::from_secs(5);

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

pub(super) struct FetchedBytes {
    pub(super) status: u16,
    pub(super) headers: HeaderMap,
    pub(super) body_bytes: Vec<u8>,
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

pub(super) fn header_map(headers: &[(&str, &str)]) -> Result<HeaderMap, Error> {
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

pub(super) async fn fetched(
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
