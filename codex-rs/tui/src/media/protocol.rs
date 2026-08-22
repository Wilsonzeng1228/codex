use std::env;

use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;

const ITERM2_KITTY_MIN_VERSION: (u64, u64, u64) = (3, 6, 0);
const DEFAULT_CHAT_MEDIA_PLACEHOLDER_ROWS: u16 = 4;
const MAX_CHAT_MEDIA_PLACEHOLDER_ROWS: u16 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChatMediaCapability {
    pub(crate) protocol: ImageProtocol,
    pub(crate) placeholder_rows: crate::media::MediaPlaceholderRows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageProtocol {
    Kitty,
    KittyLocalFile,
    Sixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageSupport {
    Supported(ImageProtocol),
    Unsupported(ImageUnsupportedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageUnsupportedReason {
    Tmux,
    Zellij,
    Iterm2TooOld,
    Terminal,
}

pub(crate) fn detect_image_support() -> ImageSupport {
    if env::var_os("TMUX").is_some() || env::var_os("TMUX_PANE").is_some() {
        return ImageSupport::Unsupported(ImageUnsupportedReason::Tmux);
    }

    if env::var_os("ZELLIJ").is_some()
        || env::var_os("ZELLIJ_SESSION_NAME").is_some()
        || env::var_os("ZELLIJ_VERSION").is_some()
    {
        return ImageSupport::Unsupported(ImageUnsupportedReason::Zellij);
    }

    if env::var_os("KITTY_WINDOW_ID").is_some()
        || env::var_os("WEZTERM_EXECUTABLE").is_some()
        || env::var_os("WEZTERM_VERSION").is_some()
    {
        return ImageSupport::Supported(ImageProtocol::Kitty);
    }

    image_support_for_terminal(&terminal_info())
}

pub(crate) fn chat_media_capability_override_from_env() -> Option<ChatMediaCapability> {
    let protocol = env::var("CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE").ok();
    let placeholder_rows = env::var("CODEX_TUI_MEDIA_PLACEHOLDER_ROWS").ok();
    parse_chat_media_capability_override(protocol.as_deref(), placeholder_rows.as_deref())
}

pub(crate) fn parse_chat_media_capability_override(
    protocol: Option<&str>,
    placeholder_rows: Option<&str>,
) -> Option<ChatMediaCapability> {
    let protocol = match protocol {
        Some("kitty") => ImageProtocol::Kitty,
        _ => return None,
    };
    let placeholder_rows = match placeholder_rows {
        Some(rows) => rows.parse::<u16>().ok()?,
        None => DEFAULT_CHAT_MEDIA_PLACEHOLDER_ROWS,
    };
    let placeholder_rows =
        (placeholder_rows <= MAX_CHAT_MEDIA_PLACEHOLDER_ROWS).then_some(placeholder_rows)?;
    let placeholder_rows = crate::media::MediaPlaceholderRows::try_from(placeholder_rows).ok()?;
    Some(ChatMediaCapability {
        protocol,
        placeholder_rows,
    })
}

pub(crate) fn image_support_for_terminal(info: &TerminalInfo) -> ImageSupport {
    match info.multiplexer {
        Some(Multiplexer::Tmux { .. }) => {
            return ImageSupport::Unsupported(ImageUnsupportedReason::Tmux);
        }
        Some(Multiplexer::Zellij { .. }) => {
            return ImageSupport::Unsupported(ImageUnsupportedReason::Zellij);
        }
        None => {}
    }

    if supports_iterm2_kitty_graphics(info) {
        return ImageSupport::Supported(ImageProtocol::KittyLocalFile);
    }

    if is_iterm2_terminal(info) {
        return ImageSupport::Unsupported(ImageUnsupportedReason::Iterm2TooOld);
    }

    if supports_kitty_graphics(info) {
        return ImageSupport::Supported(ImageProtocol::Kitty);
    }

    if supports_sixel(info) {
        return ImageSupport::Supported(ImageProtocol::Sixel);
    }

    ImageSupport::Unsupported(ImageUnsupportedReason::Terminal)
}

fn supports_iterm2_kitty_graphics(info: &TerminalInfo) -> bool {
    is_iterm2_terminal(info)
        && version_is_at_least(
            info.version.as_deref(),
            /*minimum*/ ITERM2_KITTY_MIN_VERSION,
        )
}

fn is_iterm2_terminal(info: &TerminalInfo) -> bool {
    matches!(info.name, TerminalName::Iterm2)
        || terminal_field_contains(info.term_program.as_deref(), "iterm")
}

fn supports_kitty_graphics(info: &TerminalInfo) -> bool {
    matches!(
        info.name,
        TerminalName::Ghostty | TerminalName::Kitty | TerminalName::WezTerm
    ) || terminal_field_contains(info.term.as_deref(), "kitty")
        || terminal_field_contains(info.term.as_deref(), "ghostty")
        || terminal_field_contains(info.term.as_deref(), "wezterm")
        || terminal_field_contains(info.term_program.as_deref(), "kitty")
        || terminal_field_contains(info.term_program.as_deref(), "ghostty")
        || terminal_field_contains(info.term_program.as_deref(), "wezterm")
}

fn supports_sixel(info: &TerminalInfo) -> bool {
    matches!(info.name, TerminalName::WindowsTerminal)
        || terminal_field_contains(info.term.as_deref(), "sixel")
        || terminal_field_contains(info.term.as_deref(), "mlterm")
        || terminal_field_contains(info.term.as_deref(), "foot")
}

fn terminal_field_contains(value: Option<&str>, needle: &str) -> bool {
    value.is_some_and(|value| value.to_ascii_lowercase().contains(needle))
}

fn version_is_at_least(version: Option<&str>, minimum: (u64, u64, u64)) -> bool {
    parse_dotted_version(version).is_some_and(|version| version >= minimum)
}

pub(crate) fn parse_dotted_version(version: Option<&str>) -> Option<(u64, u64, u64)> {
    let version = version?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((major, minor, patch))
}
