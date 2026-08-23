use std::net::IpAddr;
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
        Some(Host::Ipv4(address)) => is_non_public_ip(IpAddr::V4(address)),
        Some(Host::Ipv6(address)) => is_non_public_ip(IpAddr::V6(address)),
        None => true,
    };
    if unsafe_host {
        return Err(ImageSourceError::UnsafeHost(host_text));
    }

    Ok(ImageSource::Https(url))
}

pub(super) fn is_non_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_non_public_ipv4(address),
        IpAddr::V6(address) => is_non_public_ipv6(address),
    }
}

fn is_non_public_ipv4(address: Ipv4Addr) -> bool {
    address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_multicast()
        || address.is_broadcast()
        || ipv4_in_cidr(address, [0, 0, 0, 0], /*prefix*/ 8)
        || ipv4_in_cidr(address, [100, 64, 0, 0], /*prefix*/ 10)
        || ipv4_in_cidr(address, [192, 0, 0, 0], /*prefix*/ 24)
        || ipv4_in_cidr(address, [192, 0, 2, 0], /*prefix*/ 24)
        || ipv4_in_cidr(address, [198, 18, 0, 0], /*prefix*/ 15)
        || ipv4_in_cidr(address, [198, 51, 100, 0], /*prefix*/ 24)
        || ipv4_in_cidr(address, [203, 0, 113, 0], /*prefix*/ 24)
        || ipv4_in_cidr(address, [240, 0, 0, 0], /*prefix*/ 4)
}

fn ipv4_in_cidr(address: Ipv4Addr, base: [u8; 4], prefix: u8) -> bool {
    let address = u32::from(address);
    let base = u32::from(Ipv4Addr::from(base));
    let mask = u32::MAX << (32 - prefix);
    (address & mask) == (base & mask)
}

fn is_non_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(address) = address.to_ipv4() {
        return is_non_public_ipv4(address);
    }

    let segments = address.segments();
    address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || segments[0] & 0xffc0 == 0xfec0
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
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
