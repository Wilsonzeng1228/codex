mod image;
mod protocol;
mod resolver;

#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod resolver_tests;

pub(crate) use image::MediaId;
pub(crate) use image::kitty_delete_image;
pub(crate) use image::kitty_transmit_png_file_with_id;
pub(crate) use image::kitty_transmit_png_with_id;
pub(crate) use protocol::ImageProtocol;
pub(crate) use protocol::ImageSupport;
pub(crate) use protocol::ImageUnsupportedReason;
pub(crate) use protocol::detect_image_support;
#[cfg(test)]
pub(crate) use protocol::image_support_for_terminal;
#[cfg(test)]
pub(crate) use protocol::parse_dotted_version;
#[cfg(test)]
pub(crate) use resolver::ImageSource;
#[cfg(test)]
pub(crate) use resolver::ImageSourceError;
pub(crate) use resolver::resolve_image_source;
