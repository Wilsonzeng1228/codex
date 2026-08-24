//! Session-local rich-media command handling.
//!
//! The TUI owns terminal protocol state and placement retirement, while `App` owns the
//! source-backed transcript rebuild and the user-visible status card. Keeping both operations in
//! one command path prevents a runtime toggle from leaving stale terminal images or placeholder
//! rows behind.

use super::*;

impl App {
    pub(super) fn apply_rich_media_action(
        &mut self,
        tui: &mut tui::Tui,
        action: RichMediaAction,
    ) -> Result<()> {
        let changed = match action {
            RichMediaAction::Status => false,
            RichMediaAction::Enable => tui.set_chat_media_enabled(/*enabled*/ true),
            RichMediaAction::Disable => tui.set_chat_media_enabled(/*enabled*/ false),
        };

        if matches!(action, RichMediaAction::Enable | RichMediaAction::Disable) && changed {
            let terminal_width = tui.terminal.last_known_screen_size.into();
            self.reflow_transcript_now(tui, terminal_width)?;
        }

        let status = tui.chat_media_runtime_status();
        let cell = rich_media_status_cell(status, action, changed);
        self.insert_history_cell(tui, Box::new(cell));
        tui.frame_requester().schedule_frame();
        Ok(())
    }
}

fn rich_media_status_cell(
    status: crate::media::ChatMediaRuntimeStatus,
    action: RichMediaAction,
    changed: bool,
) -> history_cell::PlainHistoryCell {
    let state = if status.enabled {
        "on"
    } else if status.available.is_some() {
        "off"
    } else {
        "unavailable"
    };
    let protocol = status
        .available
        .map(|capability| protocol_label(capability.protocol))
        .unwrap_or("not detected");
    let placeholder_rows = status
        .available
        .map(|capability| capability.placeholder_rows.get().to_string())
        .unwrap_or_else(|| "-".to_string());
    let mut lines: Vec<Line<'static>> = vec![
        vec!["• ".dim(), format!("Rich media: {state}").into()].into(),
        format!("  Protocol: {protocol}").into(),
        format!("  Placeholder rows: {placeholder_rows}").into(),
        "  LaTeX renderer: RaTeX (ready)".into(),
        "  Remote images: HTTPS public addresses only".into(),
        "  Runtime command: /rich-media [status|on|off]".into(),
    ];
    if matches!(action, RichMediaAction::Enable) && !changed {
        lines.push("  Note: no supported terminal image protocol was detected".into());
    }
    history_cell::PlainHistoryCell::new(lines)
}

fn protocol_label(protocol: crate::media::ImageProtocol) -> &'static str {
    match protocol {
        crate::media::ImageProtocol::Iterm2Inline => "iTerm2 inline",
        crate::media::ImageProtocol::Kitty => "Kitty direct data",
        crate::media::ImageProtocol::KittyLocalFile => "Kitty local file",
        crate::media::ImageProtocol::Sixel => "Sixel",
    }
}
