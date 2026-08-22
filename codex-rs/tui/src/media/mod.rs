mod image;
mod layout;
mod node;
mod placement;
mod protocol;
mod resolver;
mod terminal_writer;

#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod node_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod resolver_tests;
#[cfg(test)]
mod terminal_writer_tests;

pub(crate) use image::MediaId;
pub(crate) use image::kitty_delete_image;
pub(crate) use image::kitty_transmit_png_file_with_id;
pub(crate) use image::kitty_transmit_png_with_id;
pub(crate) use layout::MediaLayout;
pub(crate) use layout::MediaPlaceholderRows;
pub(crate) use layout::MediaPlacementRequest;
pub(crate) use node::MediaNode;
pub(crate) use node::extract_media_nodes;
pub(crate) use placement::AnchoredMediaPlacementRequest;
#[cfg(test)]
pub(crate) use placement::MediaAnchor;
pub(crate) use placement::MediaCellId;
pub(crate) use placement::MediaPlacementRegistry;
pub(crate) use placement::MediaPlacementUpdate;
pub(crate) use protocol::ImageProtocol;
pub(crate) use protocol::ImageSupport;
pub(crate) use protocol::ImageUnsupportedReason;
pub(crate) use protocol::chat_media_capability_override_from_env;
pub(crate) use protocol::detect_image_support;
#[cfg(test)]
pub(crate) use protocol::image_support_for_terminal;
#[cfg(test)]
pub(crate) use protocol::parse_chat_media_capability_override;
#[cfg(test)]
pub(crate) use protocol::parse_dotted_version;
pub(crate) use resolver::ImageSource;
#[cfg(test)]
pub(crate) use resolver::ImageSourceError;
pub(crate) use resolver::resolve_image_source;
pub(crate) use terminal_writer::PreparedKittyPlacement;
pub(crate) use terminal_writer::prepare_kitty_placement_update;
pub(crate) use terminal_writer::write_kitty_placement_update;
