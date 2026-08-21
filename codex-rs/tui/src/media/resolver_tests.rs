use std::path::PathBuf;

use super::ImageSource;
use super::ImageSourceError;
use super::resolve_image_source;

#[test]
fn accepts_native_absolute_paths_and_file_uris() {
    let native_path = if cfg!(windows) {
        PathBuf::from(r"D:\course\assets\control loop.png")
    } else {
        PathBuf::from("/course/assets/control loop.png")
    };
    let file_uri = if cfg!(windows) {
        "file:///D:/course/assets/control%20loop.png"
    } else {
        "file:///course/assets/control%20loop.png"
    };
    let expected = ImageSource::Local(native_path.clone());

    assert_eq!(
        resolve_image_source(native_path.to_str().unwrap()).unwrap(),
        expected
    );
    assert_eq!(resolve_image_source(file_uri).unwrap(), expected);
}

#[test]
fn accepts_windows_drive_paths_without_expanding_variables() {
    assert_eq!(
        resolve_image_source("D:/course/assets/diagram.png").unwrap(),
        ImageSource::Local(PathBuf::from("D:/course/assets/diagram.png"))
    );
}

#[test]
fn accepts_public_https_without_credentials() {
    let source = resolve_image_source("https://example.org/diagram.png?theme=dark").unwrap();

    assert_eq!(
        source,
        ImageSource::Https(url::Url::parse("https://example.org/diagram.png?theme=dark").unwrap())
    );
}

#[test]
fn rejects_unsafe_or_ambiguous_sources() {
    for (source, expected) in [
        (
            "diagram.png",
            ImageSourceError::RelativePath("diagram.png".to_string()),
        ),
        (
            "%USERPROFILE%/diagram.png",
            ImageSourceError::RelativePath("%USERPROFILE%/diagram.png".to_string()),
        ),
        (
            "http://example.org/diagram.png",
            ImageSourceError::UnsupportedScheme("http".to_string()),
        ),
        (
            "https://user:secret@example.org/diagram.png",
            ImageSourceError::CredentialsNotAllowed,
        ),
        (
            "https://localhost/diagram.png",
            ImageSourceError::UnsafeHost("localhost".to_string()),
        ),
        (
            "https://127.0.0.1/diagram.png",
            ImageSourceError::UnsafeHost("127.0.0.1".to_string()),
        ),
        (
            "https://192.168.1.10/diagram.png",
            ImageSourceError::UnsafeHost("192.168.1.10".to_string()),
        ),
        (
            "https://[fe80::1]/diagram.png",
            ImageSourceError::UnsafeHost("[fe80::1]".to_string()),
        ),
    ] {
        assert_eq!(
            resolve_image_source(source),
            Err(expected),
            "source: {source}"
        );
    }
}
