use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;

use super::ImageProtocol;
use super::ImageSupport;
use super::ImageUnsupportedReason;
use super::image_support_for_terminal;

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
        ImageSupport::Supported(ImageProtocol::Kitty)
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
