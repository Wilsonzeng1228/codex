use std::collections::VecDeque;
use std::future::pending;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use futures::stream;
use image::DynamicImage;
use pretty_assertions::assert_eq;

use super::local_loader as local;
use super::remote_loader as remote;
use super::resolver as source;

const IMAGE_URL: &str = "https://images.example/a.png";
type DnsAnswer = Result<Vec<IpAddr>, String>;

#[derive(Clone)]
struct FakeDns {
    answers: Arc<Mutex<VecDeque<DnsAnswer>>>,
    lookups: Arc<Mutex<Vec<(String, u16)>>>,
}

impl FakeDns {
    fn new(answers: impl IntoIterator<Item = DnsAnswer>) -> Self {
        Self {
            answers: Arc::new(Mutex::new(answers.into_iter().collect())),
            lookups: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl remote::RemoteImageDnsResolver for FakeDns {
    fn resolve(
        &self,
        host: String,
        port: u16,
    ) -> impl Future<Output = Result<Vec<IpAddr>, String>> + Send {
        let answers = Arc::clone(&self.answers);
        let lookups = Arc::clone(&self.lookups);
        async move {
            lookups.lock().unwrap().push((host, port));
            answers
                .lock()
                .unwrap()
                .pop_front()
                .expect("fake DNS answer")
        }
    }
}

#[derive(Clone)]
struct FakeHttp {
    responses: Arc<Mutex<VecDeque<Result<remote::RemoteImageHttpResponse, String>>>>,
    requests: Arc<Mutex<Vec<remote::RemoteImageHttpRequest>>>,
}

impl FakeHttp {
    fn new(
        responses: impl IntoIterator<Item = Result<remote::RemoteImageHttpResponse, String>>,
    ) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl remote::RemoteImageHttpClient for FakeHttp {
    fn get(
        &self,
        request: remote::RemoteImageHttpRequest,
    ) -> impl Future<Output = Result<remote::RemoteImageHttpResponse, String>> + Send {
        let responses = Arc::clone(&self.responses);
        let requests = Arc::clone(&self.requests);
        async move {
            requests.lock().unwrap().push(request);
            responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("fake HTTP response")
        }
    }
}

#[derive(Clone, Copy)]
struct PendingHttp;

impl remote::RemoteImageHttpClient for PendingHttp {
    fn get(
        &self,
        _request: remote::RemoteImageHttpRequest,
    ) -> impl Future<Output = Result<remote::RemoteImageHttpResponse, String>> + Send {
        pending()
    }
}

fn public_ip() -> IpAddr {
    "8.8.8.8".parse().unwrap()
}

fn limits() -> remote::RemoteImageLimits {
    remote::RemoteImageLimits {
        max_redirects: 2,
        max_response_bytes: 1024 * 1024,
        dns_timeout: Duration::from_millis(50),
        connect_timeout: Duration::from_millis(17),
        read_timeout: Duration::from_millis(20),
        total_timeout: Duration::from_millis(100),
        image_limits: local::LocalImageLimits {
            max_source_bytes: 1024 * 1024,
            max_dimension: 1024,
            max_decoded_pixels: 1024 * 1024,
            max_output_dimension: 8,
            max_prepared_bytes: 1024 * 1024,
            max_cache_entries: 0,
            max_cache_bytes: 0,
            load_timeout: Duration::from_secs(1),
        },
    }
}

fn render_params() -> local::LocalImageRenderParams {
    local::LocalImageRenderParams::new(/*max_width*/ 8, /*max_height*/ 8)
}

fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let mut encoded = std::io::Cursor::new(Vec::new());
    DynamicImage::new_rgba8(width, height)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("encode PNG fixture");
    encoded.into_inner()
}

fn response(
    status: u16,
    location: Option<&str>,
    content_length: Option<u64>,
    chunks: impl IntoIterator<Item = Result<Vec<u8>, String>>,
) -> remote::RemoteImageHttpResponse {
    let chunks: Vec<_> = chunks.into_iter().collect();
    remote::RemoteImageHttpResponse::new(
        status,
        location.map(str::to_string),
        content_length,
        stream::iter(chunks),
    )
}

fn redirect(status: u16, location: &str) -> remote::RemoteImageHttpResponse {
    response(status, Some(location), Some(0), [])
}

fn ok(bytes: Vec<u8>) -> remote::RemoteImageHttpResponse {
    let size = bytes.len() as u64;
    response(200, None, Some(size), [Ok(bytes)])
}

async fn download_error<D, H>(
    loader: &remote::RemoteImageLoader<D, H>,
    source: &str,
) -> remote::RemoteImageDownloadError
where
    D: remote::RemoteImageDnsResolver,
    H: remote::RemoteImageHttpClient,
{
    loader
        .download(source, render_params())
        .await
        .expect_err("download should fail")
}

#[tokio::test]
async fn system_dns_resolver_returns_only_localhost_addresses_for_localhost() {
    let resolver = remote::SystemRemoteImageDnsResolver::new();
    let addresses = remote::RemoteImageDnsResolver::resolve(
        &resolver,
        "localhost".to_string(),
        /*port*/ 443,
    )
    .await
    .expect("resolve localhost through the system DNS resolver");

    assert!(!addresses.is_empty());
    assert!(addresses.into_iter().all(|address| address.is_loopback()));
}

#[tokio::test]
async fn fake_ip_dns_uses_fallback_without_bypassing_other_private_answers() {
    let fake_ipv4 = "198.18.0.50".parse().unwrap();
    let fake_ipv6 = "fdfe:dcba:9876::30".parse().unwrap();
    let primary = FakeDns::new([Ok(vec![fake_ipv4, fake_ipv6])]);
    let fallback = FakeDns::new([Ok(vec![public_ip()])]);
    let resolver =
        remote::FakeIpFallbackRemoteImageDnsResolver::new(primary.clone(), fallback.clone());

    let addresses = remote::RemoteImageDnsResolver::resolve(
        &resolver,
        "images.example".to_string(),
        /*port*/ 443,
    )
    .await
    .expect("replace known Fake-IP answers with fallback DNS answers");

    assert_eq!(addresses, vec![public_ip()]);
    assert_eq!(primary.lookups.lock().unwrap().len(), 1);
    assert_eq!(fallback.lookups.lock().unwrap().len(), 1);

    let private_ip = "10.0.0.8".parse().unwrap();
    let primary = FakeDns::new([Ok(vec![private_ip])]);
    let fallback = FakeDns::new([Ok(vec![public_ip()])]);
    let resolver =
        remote::FakeIpFallbackRemoteImageDnsResolver::new(primary.clone(), fallback.clone());

    let addresses = remote::RemoteImageDnsResolver::resolve(
        &resolver,
        "internal.example".to_string(),
        /*port*/ 443,
    )
    .await
    .expect("preserve non-Fake-IP private answers for the policy layer to reject");

    assert_eq!(addresses, vec![private_ip]);
    assert_eq!(primary.lookups.lock().unwrap().len(), 1);
    assert!(fallback.lookups.lock().unwrap().is_empty());

    let primary = FakeDns::new([Ok(vec![public_ip()])]);
    let fallback = FakeDns::new([Ok(vec!["1.1.1.1".parse().unwrap()])]);
    let resolver =
        remote::FakeIpFallbackRemoteImageDnsResolver::new(primary.clone(), fallback.clone());

    let addresses = remote::RemoteImageDnsResolver::resolve(
        &resolver,
        "public.example".to_string(),
        /*port*/ 443,
    )
    .await
    .expect("use normal public system DNS answers without a fallback lookup");

    assert_eq!(addresses, vec![public_ip()]);
    assert_eq!(primary.lookups.lock().unwrap().len(), 1);
    assert!(fallback.lookups.lock().unwrap().is_empty());
}

#[test]
fn dns_over_https_json_keeps_only_requested_address_records() {
    let response = br#"{
        "Status": 0,
        "Answer": [
            {"name":"images.example.","type":5,"TTL":60,"data":"cdn.example."},
            {"name":"cdn.example.","type":1,"TTL":60,"data":"8.8.8.8"},
            {"name":"cdn.example.","type":28,"TTL":60,"data":"2001:4860:4860::8888"}
        ]
    }"#;

    assert_eq!(
        remote::parse_dns_over_https_answers(response, /*record_type*/ 1).unwrap(),
        vec!["8.8.8.8".parse::<IpAddr>().unwrap()]
    );
    assert_eq!(
        remote::parse_dns_over_https_answers(response, /*record_type*/ 28).unwrap(),
        vec!["2001:4860:4860::8888".parse::<IpAddr>().unwrap()]
    );
    assert!(remote::parse_dns_over_https_answers(br#"{"Status":2}"#, 1).is_err());
    assert!(remote::parse_dns_over_https_answers(b"not json", 1).is_err());
}

#[tokio::test]
async fn remote_loader_allows_only_https_with_public_dns_answers() {
    let dns = FakeDns::new([]);
    let http = FakeHttp::new([]);
    let loader = remote::RemoteImageLoader::new(dns.clone(), http.clone(), limits());

    let scheme_error = download_error(&loader, "http://images.example/diagram.png").await;
    assert_eq!(
        scheme_error,
        remote::RemoteImageDownloadError::InvalidSource(
            source::ImageSourceError::UnsupportedScheme("http".to_string())
        )
    );
    assert!(dns.lookups.lock().unwrap().is_empty());
    assert!(http.requests.lock().unwrap().is_empty());

    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "169.254.1.1",
        "0.0.0.0",
        "100.64.0.1",
        "192.0.2.1",
        "240.0.0.1",
        "::1",
        "fc00::1",
        "fe80::1",
        "::",
    ] {
        let address = address.parse().unwrap();
        let loader = remote::RemoteImageLoader::new(
            FakeDns::new([Ok(vec![address])]),
            FakeHttp::new([]),
            limits(),
        );
        let error = download_error(&loader, IMAGE_URL).await;
        assert_eq!(
            error,
            remote::RemoteImageDownloadError::NonPublicAddress {
                host: "images.example".to_string(),
                address,
            }
        );
    }
}

#[tokio::test]
async fn every_redirect_revalidates_scheme_host_and_dns() {
    for (location, expected) in [
        (
            "http://cdn.example/diagram.png",
            remote::RemoteImageDownloadError::InvalidSource(
                source::ImageSourceError::UnsupportedScheme("http".to_string()),
            ),
        ),
        (
            "https://localhost/diagram.png",
            remote::RemoteImageDownloadError::InvalidSource(source::ImageSourceError::UnsafeHost(
                "localhost".to_string(),
            )),
        ),
    ] {
        let loader = remote::RemoteImageLoader::new(
            FakeDns::new([Ok(vec![public_ip()])]),
            FakeHttp::new([Ok(redirect(302, location))]),
            limits(),
        );
        assert_eq!(download_error(&loader, IMAGE_URL).await, expected);
    }

    let private_redirect_ip = "192.168.1.2".parse().unwrap();
    let dns = FakeDns::new([Ok(vec![public_ip()]), Ok(vec![private_redirect_ip])]);
    let loader = remote::RemoteImageLoader::new(
        dns.clone(),
        FakeHttp::new([Ok(redirect(307, "https://cdn.example/diagram.png"))]),
        limits(),
    );
    assert_eq!(
        download_error(&loader, IMAGE_URL).await,
        remote::RemoteImageDownloadError::NonPublicAddress {
            host: "cdn.example".to_string(),
            address: private_redirect_ip,
        }
    );
    assert_eq!(
        *dns.lookups.lock().unwrap(),
        vec![
            ("images.example".to_string(), 443),
            ("cdn.example".to_string(), 443),
        ]
    );
}

#[tokio::test]
async fn remote_loader_limits_redirect_count() {
    let mut test_limits = limits();
    test_limits.max_redirects = 1;
    let http = FakeHttp::new([
        Ok(redirect(302, "https://first.example/a.png")),
        Ok(redirect(302, "https://second.example/a.png")),
    ]);
    let loader = remote::RemoteImageLoader::new(
        FakeDns::new([Ok(vec![public_ip()]), Ok(vec![public_ip()])]),
        http.clone(),
        test_limits,
    );

    assert_eq!(
        download_error(&loader, IMAGE_URL).await,
        remote::RemoteImageDownloadError::TooManyRedirects { max: 1 }
    );
    assert_eq!(http.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn remote_loader_sets_connect_timeout_and_enforces_read_and_total_timeouts() {
    let png = png_fixture(/*width*/ 2, /*height*/ 1);
    let http = FakeHttp::new([Ok(ok(png))]);
    let test_limits = limits();
    let loader = remote::RemoteImageLoader::new(
        FakeDns::new([Ok(vec![public_ip()])]),
        http.clone(),
        test_limits,
    );
    loader
        .download(IMAGE_URL, render_params())
        .await
        .expect("download within timeouts");
    {
        let requests = http.requests.lock().unwrap();
        assert_eq!(requests[0].connect_timeout, test_limits.connect_timeout);
        assert_eq!(requests[0].resolved_addrs[0].ip(), public_ip());
    }

    let mut read_limits = limits();
    read_limits.read_timeout = Duration::from_millis(2);
    let read_loader = remote::RemoteImageLoader::new(
        FakeDns::new([Ok(vec![public_ip()])]),
        FakeHttp::new([Ok(remote::RemoteImageHttpResponse::new(
            200,
            None,
            None,
            stream::pending(),
        ))]),
        read_limits,
    );
    assert_eq!(
        download_error(&read_loader, IMAGE_URL).await,
        remote::RemoteImageDownloadError::ReadTimeout { milliseconds: 2 }
    );

    let mut total_limits = limits();
    total_limits.total_timeout = Duration::from_millis(2);
    let total_loader = remote::RemoteImageLoader::new(
        FakeDns::new([Ok(vec![public_ip()])]),
        PendingHttp,
        total_limits,
    );
    assert_eq!(
        download_error(&total_loader, IMAGE_URL).await,
        remote::RemoteImageDownloadError::TotalTimeout { milliseconds: 2 }
    );
}

#[tokio::test]
async fn oversized_stream_stops_without_polling_another_chunk() {
    let polls = Arc::new(AtomicUsize::new(0));
    let stream_polls = Arc::clone(&polls);
    let body = stream::unfold(0, move |state| {
        let stream_polls = Arc::clone(&stream_polls);
        async move {
            stream_polls.fetch_add(1, Ordering::SeqCst);
            match state {
                0 => Some((Ok(b"12345".to_vec()), 1)),
                _ => panic!("body must not be polled after exceeding the limit"),
            }
        }
    });
    let mut test_limits = limits();
    test_limits.max_response_bytes = 4;
    let loader = remote::RemoteImageLoader::new(
        FakeDns::new([Ok(vec![public_ip()])]),
        FakeHttp::new([Ok(remote::RemoteImageHttpResponse::new(
            200, None, None, body,
        ))]),
        test_limits,
    );

    assert_eq!(
        download_error(&loader, IMAGE_URL).await,
        remote::RemoteImageDownloadError::ResponseTooLarge { size: 5, max: 4 }
    );
    assert_eq!(polls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn downloaded_bytes_use_existing_magic_decode_and_pixel_limits() {
    let malformed_loader = remote::RemoteImageLoader::new(
        FakeDns::new([Ok(vec![public_ip()])]),
        FakeHttp::new([Ok(ok(b"not-an-image".to_vec()))]),
        limits(),
    );
    assert!(matches!(
        malformed_loader.download(IMAGE_URL, render_params()).await,
        Err(remote::RemoteImageDownloadError::Image(
            local::LocalImageLoadError::InvalidImage { .. }
        ))
    ));

    let mut pixel_limits = limits();
    pixel_limits.image_limits.max_decoded_pixels = 1;
    let png = png_fixture(/*width*/ 2, /*height*/ 1);
    let pixel_loader = remote::RemoteImageLoader::new(
        FakeDns::new([Ok(vec![public_ip()])]),
        FakeHttp::new([Ok(ok(png))]),
        pixel_limits,
    );
    assert_eq!(
        download_error(&pixel_loader, IMAGE_URL).await,
        remote::RemoteImageDownloadError::Image(local::LocalImageLoadError::PixelLimitExceeded {
            width: 2,
            height: 1,
            max_pixels: 1,
        })
    );
}

#[tokio::test]
async fn pinned_http_adapter_connects_to_validated_address_without_following_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind HTTP fixture");
    let address = listener.local_addr().expect("fixture address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept HTTP request");
        let mut request = Vec::new();
        loop {
            let mut chunk = [0; 1024];
            let size = stream.read(&mut chunk).expect("read HTTP request");
            if size == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..size]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: http://redirect.invalid/next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write HTTP response");
        String::from_utf8(request).expect("HTTP request is UTF-8")
    });

    let client = remote::PinnedRemoteImageHttpClient::new();
    let url = format!("http://pinned.invalid:{}/start", address.port())
        .parse()
        .expect("fixture URL");
    remote::RemoteImageHttpClient::get(
        &client,
        remote::RemoteImageHttpRequest {
            url,
            resolved_addrs: vec![address],
            connect_timeout: Duration::from_secs(2),
        },
    )
    .await
    .expect("return redirect response without following it");

    let request = server.join().expect("HTTP fixture thread");
    assert!(request.starts_with("GET /start HTTP/1.1\r\n"));
    assert!(request.contains(&format!("\r\nhost: pinned.invalid:{}\r\n", address.port())));
}
