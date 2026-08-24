use std::future::Future;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::time::Duration;

use codex_http_client::HttpClientBuilder;
use serde::Deserialize;
use thiserror::Error;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use url::Url;

use super::local_loader as local;
use super::resolver as source;

const DEFAULT_MAX_REDIRECTS: usize = 5;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_DNS_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_TOTAL_TIMEOUT: Duration = Duration::from_secs(15);
const DNS_OVER_HTTPS_ENDPOINT: &str = "https://dns.google/resolve";
const MAX_DNS_OVER_HTTPS_RESPONSE_BYTES: usize = 64 * 1024;

/// Resolves one remote-image host so policy can validate every address before a request is sent.
pub(crate) trait RemoteImageDnsResolver: Clone + Send + Sync + 'static {
    fn resolve(
        &self,
        host: String,
        port: u16,
    ) -> impl Future<Output = Result<Vec<IpAddr>, String>> + Send;
}

/// Resolves remote-image hosts through the operating system resolver.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SystemRemoteImageDnsResolver;

impl SystemRemoteImageDnsResolver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl RemoteImageDnsResolver for SystemRemoteImageDnsResolver {
    async fn resolve(&self, host: String, port: u16) -> Result<Vec<IpAddr>, String> {
        tokio::task::spawn_blocking(move || {
            let mut addresses = Vec::new();
            let resolved = (host.as_str(), port)
                .to_socket_addrs()
                .map_err(|error| error.to_string())?;
            for address in resolved {
                let address = address.ip();
                if !addresses.contains(&address) {
                    addresses.push(address);
                }
            }
            Ok(addresses)
        })
        .await
        .map_err(|error| format!("system DNS worker failed: {error}"))?
    }
}

/// Uses a secondary resolver only when the primary returns a known transparent-proxy Fake-IP.
///
/// Normal public system answers stay on the primary path. Other private or special-use answers are
/// deliberately preserved so the remote-image policy rejects them rather than bypassing them.
#[derive(Clone, Debug)]
pub(crate) struct FakeIpFallbackRemoteImageDnsResolver<P, F> {
    primary: P,
    fallback: F,
}

impl<P, F> FakeIpFallbackRemoteImageDnsResolver<P, F> {
    pub(crate) const fn new(primary: P, fallback: F) -> Self {
        Self { primary, fallback }
    }
}

impl<P, F> RemoteImageDnsResolver for FakeIpFallbackRemoteImageDnsResolver<P, F>
where
    P: RemoteImageDnsResolver,
    F: RemoteImageDnsResolver,
{
    async fn resolve(&self, host: String, port: u16) -> Result<Vec<IpAddr>, String> {
        let addresses = self.primary.resolve(host.clone(), port).await?;
        if !addresses.is_empty() && addresses.iter().copied().all(is_known_proxy_fake_ip) {
            tracing::debug!(%host, "system DNS returned proxy Fake-IP; using DNS-over-HTTPS fallback");
            self.fallback.resolve(host, port).await
        } else {
            Ok(addresses)
        }
    }
}

/// Resolves through a fixed HTTPS DNS endpoint when a transparent proxy hides real addresses.
///
/// The endpoint itself uses ordinary TLS hostname verification. Returned addresses still pass
/// through the same public-address policy and are pinned by the image HTTP adapter.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DnsOverHttpsRemoteImageDnsResolver;

impl DnsOverHttpsRemoteImageDnsResolver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl RemoteImageDnsResolver for DnsOverHttpsRemoteImageDnsResolver {
    async fn resolve(&self, host: String, _port: u16) -> Result<Vec<IpAddr>, String> {
        let client = HttpClientBuilder::new()
            .without_redirects()
            .without_request_logging()
            .connect_timeout(DEFAULT_DNS_TIMEOUT)
            .build_direct()
            .map_err(|error| format!("build DNS-over-HTTPS client: {error}"))?;
        let (ipv4, ipv6) = tokio::try_join!(
            resolve_dns_over_https_record(&client, &host, /*record_type*/ 1),
            resolve_dns_over_https_record(&client, &host, /*record_type*/ 28),
        )?;
        let mut addresses = ipv4;
        for address in ipv6 {
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        Ok(addresses)
    }
}

pub(crate) type ProductionRemoteImageDnsResolver = FakeIpFallbackRemoteImageDnsResolver<
    SystemRemoteImageDnsResolver,
    DnsOverHttpsRemoteImageDnsResolver,
>;

#[derive(Deserialize)]
struct DnsOverHttpsResponse {
    #[serde(rename = "Status")]
    status: u16,
    #[serde(rename = "Answer", default)]
    answers: Vec<DnsOverHttpsAnswer>,
}

#[derive(Deserialize)]
struct DnsOverHttpsAnswer {
    #[serde(rename = "type")]
    record_type: u16,
    data: String,
}

async fn resolve_dns_over_https_record(
    client: &codex_http_client::HttpClient,
    host: &str,
    record_type: u16,
) -> Result<Vec<IpAddr>, String> {
    let mut url = Url::parse(DNS_OVER_HTTPS_ENDPOINT)
        .map_err(|error| format!("invalid DNS-over-HTTPS endpoint: {error}"))?;
    let record_type_name = match record_type {
        1 => "A",
        28 => "AAAA",
        _ => return Err(format!("unsupported DNS record type {record_type}")),
    };
    url.query_pairs_mut()
        .append_pair("name", host)
        .append_pair("type", record_type_name);
    let response = client
        .get(url)
        .header("accept", "application/dns-json")
        .timeout(DEFAULT_DNS_TIMEOUT)
        .send()
        .await
        .map_err(|error| format!("DNS-over-HTTPS request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("DNS-over-HTTPS returned HTTP status {status}"));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_DNS_OVER_HTTPS_RESPONSE_BYTES as u64)
    {
        return Err("DNS-over-HTTPS response exceeded 65536 bytes".to_string());
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("read DNS-over-HTTPS response: {error}"))?;
        if body.len().saturating_add(chunk.len()) > MAX_DNS_OVER_HTTPS_RESPONSE_BYTES {
            return Err("DNS-over-HTTPS response exceeded 65536 bytes".to_string());
        }
        body.extend_from_slice(&chunk);
    }
    parse_dns_over_https_answers(&body, record_type)
}

pub(crate) fn parse_dns_over_https_answers(
    body: &[u8],
    record_type: u16,
) -> Result<Vec<IpAddr>, String> {
    let response: DnsOverHttpsResponse = serde_json::from_slice(body)
        .map_err(|error| format!("invalid DNS-over-HTTPS JSON: {error}"))?;
    if response.status != 0 {
        return Err(format!(
            "DNS-over-HTTPS returned DNS status {}",
            response.status
        ));
    }
    response
        .answers
        .into_iter()
        .filter(|answer| answer.record_type == record_type)
        .map(|answer| {
            answer
                .data
                .parse::<IpAddr>()
                .map_err(|error| format!("invalid DNS-over-HTTPS address {}: {error}", answer.data))
        })
        .collect()
}

fn is_known_proxy_fake_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let octets = address.octets();
            octets[0] == 198 && matches!(octets[1], 18 | 19)
        }
        IpAddr::V6(address) => address.octets()[..8] == [0xfd, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0, 0],
    }
}

/// Sends one redirect-disabled request to the already validated addresses in the request.
///
/// Implementations must connect only to `RemoteImageHttpRequest::resolved_addrs`; resolving the
/// hostname again would reopen DNS rebinding between policy validation and connection setup.
pub(crate) trait RemoteImageHttpClient: Clone + Send + Sync + 'static {
    fn get(
        &self,
        request: RemoteImageHttpRequest,
    ) -> impl Future<Output = Result<RemoteImageHttpResponse, String>> + Send;
}

/// Sends remote-image requests directly to addresses already approved by the policy layer.
///
/// Direct routing is required here because an HTTP proxy could resolve the target hostname again
/// and bypass the checked addresses. Redirects stay visible so the policy layer can validate each
/// hop before sending it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PinnedRemoteImageHttpClient;

impl PinnedRemoteImageHttpClient {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl RemoteImageHttpClient for PinnedRemoteImageHttpClient {
    async fn get(
        &self,
        request: RemoteImageHttpRequest,
    ) -> Result<RemoteImageHttpResponse, String> {
        let host = request
            .url
            .host_str()
            .ok_or_else(|| "remote image URL is missing a host".to_string())?;
        let client = HttpClientBuilder::new()
            .without_redirects()
            .without_request_logging()
            .connect_timeout(request.connect_timeout)
            .resolve_to_addrs(host, &request.resolved_addrs)
            .build_direct()
            .map_err(|error| error.to_string())?;
        let response = client
            .get(request.url)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get("location")
            .map(|value| value.to_str().map(str::to_string))
            .transpose()
            .map_err(|error| format!("invalid redirect Location header: {error}"))?;
        let content_length = response.content_length();
        let body = response.bytes_stream().map(|chunk| {
            chunk
                .map(|bytes| bytes.to_vec())
                .map_err(|error| error.to_string())
        });
        Ok(RemoteImageHttpResponse::new(
            status,
            location,
            content_length,
            body,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteImageHttpRequest {
    pub(crate) url: Url,
    pub(crate) resolved_addrs: Vec<SocketAddr>,
    pub(crate) connect_timeout: Duration,
}

type RemoteImageBody = Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>>;

pub(crate) struct RemoteImageHttpResponse {
    status: u16,
    location: Option<String>,
    content_length: Option<u64>,
    body: RemoteImageBody,
}

impl RemoteImageHttpResponse {
    pub(crate) fn new(
        status: u16,
        location: Option<String>,
        content_length: Option<u64>,
        body: impl Stream<Item = Result<Vec<u8>, String>> + Send + 'static,
    ) -> Self {
        Self {
            status,
            location,
            content_length,
            body: Box::pin(body),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RemoteImageLimits {
    pub(crate) max_redirects: usize,
    pub(crate) max_response_bytes: usize,
    pub(crate) dns_timeout: Duration,
    pub(crate) connect_timeout: Duration,
    pub(crate) read_timeout: Duration,
    pub(crate) total_timeout: Duration,
    pub(crate) image_limits: local::LocalImageLimits,
}

impl Default for RemoteImageLimits {
    fn default() -> Self {
        Self {
            max_redirects: DEFAULT_MAX_REDIRECTS,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            dns_timeout: DEFAULT_DNS_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            total_timeout: DEFAULT_TOTAL_TIMEOUT,
            image_limits: local::LocalImageLimits::default(),
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum RemoteImageDownloadError {
    #[error(transparent)]
    InvalidSource(#[from] source::ImageSourceError),
    #[error("remote image source must be HTTPS")]
    ExpectedHttps,
    #[error("DNS lookup for {host} failed: {message}")]
    Dns { host: String, message: String },
    #[error("DNS lookup for {host} timed out after {milliseconds} ms")]
    DnsTimeout { host: String, milliseconds: u64 },
    #[error("DNS lookup for {host} returned no addresses")]
    NoAddresses { host: String },
    #[error("remote image host {host} resolved to non-public address {address}")]
    NonPublicAddress { host: String, address: IpAddr },
    #[error("remote image request failed: {message}")]
    Http { message: String },
    #[error("remote image redirect is missing a Location header")]
    MissingRedirectLocation,
    #[error("remote image exceeded the redirect limit ({max})")]
    TooManyRedirects { max: usize },
    #[error("remote image returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("remote image response is too large ({size} bytes; max {max} bytes)")]
    ResponseTooLarge { size: usize, max: usize },
    #[error("remote image body read failed: {message}")]
    Read { message: String },
    #[error("remote image body read timed out after {milliseconds} ms")]
    ReadTimeout { milliseconds: u64 },
    #[error("remote image download timed out after {milliseconds} ms")]
    TotalTimeout { milliseconds: u64 },
    #[error(transparent)]
    Image(#[from] local::LocalImageLoadError),
}

#[derive(Clone)]
pub(crate) struct RemoteImageLoader<D, H> {
    dns: D,
    http: H,
    limits: RemoteImageLimits,
}

impl<D, H> RemoteImageLoader<D, H>
where
    D: RemoteImageDnsResolver,
    H: RemoteImageHttpClient,
{
    pub(crate) fn new(dns: D, http: H, limits: RemoteImageLimits) -> Self {
        Self { dns, http, limits }
    }

    pub(crate) async fn download(
        &self,
        source: &str,
        render_params: local::LocalImageRenderParams,
    ) -> Result<local::LoadedLocalImage, RemoteImageDownloadError> {
        match tokio::time::timeout(
            self.limits.total_timeout,
            self.download_inner(source, render_params),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(RemoteImageDownloadError::TotalTimeout {
                milliseconds: duration_millis(self.limits.total_timeout),
            }),
        }
    }

    async fn download_inner(
        &self,
        source: &str,
        render_params: local::LocalImageRenderParams,
    ) -> Result<local::LoadedLocalImage, RemoteImageDownloadError> {
        let mut url = https_url(source)?;
        let mut redirects = 0;

        loop {
            let host = url
                .host_str()
                .ok_or(source::ImageSourceError::MissingHost)?
                .to_string();
            let port = url.port_or_known_default().unwrap_or(443);
            let addresses = self.resolve_public_addresses(&host, port).await?;
            let request = RemoteImageHttpRequest {
                url: url.clone(),
                resolved_addrs: addresses,
                connect_timeout: self.limits.connect_timeout,
            };
            let mut response = self
                .http
                .get(request)
                .await
                .map_err(|message| RemoteImageDownloadError::Http { message })?;

            if is_redirect(response.status) {
                if redirects >= self.limits.max_redirects {
                    return Err(RemoteImageDownloadError::TooManyRedirects {
                        max: self.limits.max_redirects,
                    });
                }
                let location = response
                    .location
                    .as_deref()
                    .ok_or(RemoteImageDownloadError::MissingRedirectLocation)?;
                let next = url
                    .join(location)
                    .map_err(|error| RemoteImageDownloadError::Http {
                        message: format!("invalid redirect target: {error}"),
                    })?;
                url = https_url(next.as_str())?;
                redirects += 1;
                continue;
            }

            if !(200..300).contains(&response.status) {
                return Err(RemoteImageDownloadError::HttpStatus {
                    status: response.status,
                });
            }
            if response
                .content_length
                .is_some_and(|size| size > self.limits.max_response_bytes as u64)
            {
                let size = response
                    .content_length
                    .and_then(|size| usize::try_from(size).ok())
                    .unwrap_or(usize::MAX);
                return Err(RemoteImageDownloadError::ResponseTooLarge {
                    size,
                    max: self.limits.max_response_bytes,
                });
            }

            let mut bytes = Vec::new();
            loop {
                let next = tokio::time::timeout(self.limits.read_timeout, response.body.next())
                    .await
                    .map_err(|_| RemoteImageDownloadError::ReadTimeout {
                        milliseconds: duration_millis(self.limits.read_timeout),
                    })?;
                let Some(chunk) = next else {
                    break;
                };
                let chunk = chunk.map_err(|message| RemoteImageDownloadError::Read { message })?;
                let size = bytes.len().saturating_add(chunk.len());
                if size > self.limits.max_response_bytes {
                    return Err(RemoteImageDownloadError::ResponseTooLarge {
                        size,
                        max: self.limits.max_response_bytes,
                    });
                }
                bytes.extend_from_slice(&chunk);
            }

            let image_limits = self.limits.image_limits;
            let source = url.to_string();
            let task = tokio::task::spawn_blocking(move || {
                local::prepare_image_bytes(
                    bytes,
                    image_limits,
                    render_params,
                    source,
                    local::SourceFileReuse::Unavailable,
                )
            });
            let result = tokio::time::timeout(image_limits.load_timeout, task)
                .await
                .map_err(|_| local::LocalImageLoadError::Timeout {
                    milliseconds: duration_millis(image_limits.load_timeout),
                })?
                .map_err(|error| local::LocalImageLoadError::Worker {
                    message: error.to_string(),
                })?;
            return result.map_err(RemoteImageDownloadError::Image);
        }
    }

    async fn resolve_public_addresses(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, RemoteImageDownloadError> {
        let addresses = tokio::time::timeout(
            self.limits.dns_timeout,
            self.dns.resolve(host.to_string(), port),
        )
        .await
        .map_err(|_| RemoteImageDownloadError::DnsTimeout {
            host: host.to_string(),
            milliseconds: duration_millis(self.limits.dns_timeout),
        })?
        .map_err(|message| RemoteImageDownloadError::Dns {
            host: host.to_string(),
            message,
        })?;
        if addresses.is_empty() {
            return Err(RemoteImageDownloadError::NoAddresses {
                host: host.to_string(),
            });
        }
        if let Some(address) = addresses
            .iter()
            .copied()
            .find(|ip| source::is_non_public_ip(*ip))
        {
            return Err(RemoteImageDownloadError::NonPublicAddress {
                host: host.to_string(),
                address,
            });
        }
        Ok(addresses
            .into_iter()
            .map(|address| SocketAddr::new(address, port))
            .collect())
    }
}

impl RemoteImageLoader<ProductionRemoteImageDnsResolver, PinnedRemoteImageHttpClient> {
    pub(crate) fn production() -> Self {
        Self::new(
            FakeIpFallbackRemoteImageDnsResolver::new(
                SystemRemoteImageDnsResolver::new(),
                DnsOverHttpsRemoteImageDnsResolver::new(),
            ),
            PinnedRemoteImageHttpClient::new(),
            RemoteImageLimits::default(),
        )
    }
}

fn https_url(source: &str) -> Result<Url, RemoteImageDownloadError> {
    match source::resolve_image_source(source)? {
        source::ImageSource::Https(url) => Ok(url),
        source::ImageSource::Local(_) => Err(RemoteImageDownloadError::ExpectedHttps),
    }
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
