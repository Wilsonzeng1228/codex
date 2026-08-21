use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::path::PathBuf;

use codex_utils_path_uri::PathUri;
use thiserror::Error;
use url::Host;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ImageSource {
    Local(PathBuf),
    Https(Url),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum ImageSourceError {
    #[error("image path must be absolute: {0}")]
    RelativePath(String),
    #[error("unsupported image URL scheme: {0}")]
    UnsupportedScheme(String),
    #[error("invalid file URI: {uri}: {reason}")]
    InvalidFileUri { uri: String, reason: String },
    #[error("remote image URL must include a host")]
    MissingHost,
    #[error("credentials are not allowed in remote image URLs")]
    CredentialsNotAllowed,
    #[error("remote image host is not allowed: {0}")]
    UnsafeHost(String),
}

pub(crate) fn resolve_image_source(source: &str) -> Result<ImageSource, ImageSourceError> {
    let path = PathBuf::from(source);
    if path.is_absolute() || is_windows_absolute_path(source) {
        return Ok(ImageSource::Local(path));
    }

    let url = match Url::parse(source) {
        Ok(url) => url,
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            return Err(ImageSourceError::RelativePath(source.to_string()));
        }
        Err(error) => {
            return Err(ImageSourceError::InvalidFileUri {
                uri: source.to_string(),
                reason: error.to_string(),
            });
        }
    };

    match url.scheme() {
        "file" => resolve_file_uri(source),
        "https" => resolve_https_url(url),
        scheme => Err(ImageSourceError::UnsupportedScheme(scheme.to_string())),
    }
}

fn resolve_file_uri(source: &str) -> Result<ImageSource, ImageSourceError> {
    let uri = PathUri::parse(source).map_err(|error| ImageSourceError::InvalidFileUri {
        uri: source.to_string(),
        reason: error.to_string(),
    })?;
    let path = uri.to_path_buf();
    if path.is_absolute() || is_windows_absolute_path(path.to_string_lossy().as_ref()) {
        Ok(ImageSource::Local(path))
    } else {
        Err(ImageSourceError::InvalidFileUri {
            uri: source.to_string(),
            reason: "URI does not resolve to an absolute path on this host".to_string(),
        })
    }
}

fn resolve_https_url(url: Url) -> Result<ImageSource, ImageSourceError> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ImageSourceError::CredentialsNotAllowed);
    }

    let host_text = url
        .host_str()
        .ok_or(ImageSourceError::MissingHost)?
        .to_string();
    let unsafe_host = match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.');
            domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".localhost")
                || domain.to_ascii_lowercase().ends_with(".local")
        }
        Some(Host::Ipv4(address)) => is_unsafe_ipv4(address),
        Some(Host::Ipv6(address)) => is_unsafe_ipv6(address),
        None => true,
    };
    if unsafe_host {
        return Err(ImageSourceError::UnsafeHost(host_text));
    }

    Ok(ImageSource::Https(url))
}

fn is_unsafe_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_multicast()
        || octets[0] == 0
        || octets == [255, 255, 255, 255]
}

fn is_unsafe_ipv6(address: Ipv6Addr) -> bool {
    if let Some(address) = address.to_ipv4_mapped() {
        return is_unsafe_ipv4(address);
    }

    let segments = address.segments();
    address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
}

fn is_windows_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    matches!(
        bytes,
        [drive, b':', separator, ..]
            if drive.is_ascii_alphabetic() && matches!(*separator, b'/' | b'\\')
    ) || matches!(
        bytes,
        [first, second, rest @ ..]
            if matches!((*first, *second), (b'\\', b'\\') | (b'/', b'/'))
                && rest.split(|byte| matches!(*byte, b'/' | b'\\'))
                    .filter(|component| !component.is_empty())
                    .take(2)
                    .count()
                    == 2
    )
}
