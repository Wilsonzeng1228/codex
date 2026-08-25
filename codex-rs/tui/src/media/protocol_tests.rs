use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;

use super::ImageProtocol;
use super::ImageSupport;
use super::ImageUnsupportedReason;
use super::image_support_for_terminal;
use super::parse_chat_media_capability_override;

#[test]
fn default_placeholder_height_keeps_wide_image_text_readable() {
    let enabled = parse_chat_media_capability_override(Some("iterm2"), None)
        .expect("valid explicit iTerm2 override");

    assert_eq!(enabled.placeholder_rows.get(), 12);
}

#[test]
fn explicit_chat_media_override_is_bounded_and_disabled_by_default() {
    let enabled = parse_chat_media_capability_override(Some("kitty"), Some("4"))
        .expect("valid explicit Kitty override");

    assert_eq!(enabled.protocol, ImageProtocol::Kitty);
    assert_eq!(enabled.placeholder_rows.get(), 4);
    assert_eq!(
        parse_chat_media_capability_override(Some("iterm2"), Some("4"))
            .expect("valid explicit iTerm2 override")
            .protocol,
        ImageProtocol::Iterm2Inline
    );
    assert_eq!(parse_chat_media_capability_override(None, None), None);
    assert_eq!(
        parse_chat_media_capability_override(Some("kitty"), Some("0")),
        None
    );
    assert_eq!(
        parse_chat_media_capability_override(Some("kitty"), Some("33")),
        None
    );
    assert_eq!(
        parse_chat_media_capability_override(Some("sixel"), Some("4")),
        None
    );
}

#[test]
fn protocol_detection_is_shared_and_multiplexer_safe() {
    let wezterm = terminal_info(
        TerminalName::WezTerm,
        /*multiplexer*/ None,
        Some("WezTerm"),
        Some("xterm-256color"),
    );
    let windows_terminal = terminal_info(
        TerminalName::WindowsTerminal,
        /*multiplexer*/ None,
        Some("WindowsTerminal"),
        Some("xterm-256color"),
    );
    let kitty_in_tmux = terminal_info(
        TerminalName::Kitty,
        Some(Multiplexer::Tmux { version: None }),
        Some("kitty"),
        Some("xterm-kitty"),
    );

    assert_eq!(
        image_support_for_terminal(&wezterm),
        ImageSupport::Supported(if cfg!(windows) {
            ImageProtocol::Iterm2Inline
        } else {
            ImageProtocol::Kitty
        })
    );
    assert_eq!(
        image_support_for_terminal(&windows_terminal),
        ImageSupport::Supported(ImageProtocol::Sixel)
    );
    assert_eq!(
        image_support_for_terminal(&kitty_in_tmux),
        ImageSupport::Unsupported(ImageUnsupportedReason::Tmux)
    );
}

fn terminal_info(
    name: TerminalName,
    multiplexer: Option<Multiplexer>,
    term_program: Option<&str>,
    term: Option<&str>,
) -> TerminalInfo {
    TerminalInfo {
        name,
        term_program: term_program.map(str::to_string),
        version: None,
        term: term.map(str::to_string),
        multiplexer,
    }
}
