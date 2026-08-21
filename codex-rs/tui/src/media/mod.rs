mod protocol;

#[cfg(test)]
mod protocol_tests;

pub(crate) use protocol::ImageProtocol;
pub(crate) use protocol::ImageSupport;
pub(crate) use protocol::ImageUnsupportedReason;
pub(crate) use protocol::detect_image_support;
#[cfg(test)]
pub(crate) use protocol::image_support_for_terminal;
#[cfg(test)]
pub(crate) use protocol::parse_dotted_version;
