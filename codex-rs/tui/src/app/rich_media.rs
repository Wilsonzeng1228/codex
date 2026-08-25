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
        let (changed, reloaded_sources) = match action {
            RichMediaAction::Status => (false, None),
            RichMediaAction::Enable => {
                (tui.set_chat_media_enabled(/*enabled*/ true), None)
            }
            RichMediaAction::Disable => {
                (tui.set_chat_media_enabled(/*enabled*/ false), None)
            }
            RichMediaAction::ClearCache => (false, Some(tui.clear_chat_media_cache())),
        };

        if matches!(action, RichMediaAction::Enable | RichMediaAction::Disable) && changed {
            let terminal_width = tui.terminal.last_known_screen_size.into();
            self.reflow_transcript_now(tui, terminal_width)?;
        }

        let status = tui.chat_media_runtime_status();
        let cell = rich_media_status_cell(status, action, changed, reloaded_sources);
        self.insert_history_cell(tui, Box::new(cell));
        tui.frame_requester().schedule_frame();
        Ok(())
    }
}

fn rich_media_status_cell(
    status: crate::media::ChatMediaRuntimeStatus<'_>,
    action: RichMediaAction,
    changed: bool,
    reloaded_sources: Option<usize>,
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
    let last_error = status.last_error.unwrap_or("none");
    let mut lines: Vec<Line<'static>> = vec![
        vec!["• ".dim(), format!("Rich media: {state}").into()].into(),
        format!("  Terminal: {}", status.terminal).into(),
        format!("  Protocol: {protocol}").into(),
        format!("  Placeholder rows: {placeholder_rows}").into(),
        "  LaTeX renderer: RaTeX (ready)".into(),
        "  Remote images: HTTPS public addresses only".into(),
        format!("  Last render error: {last_error}").into(),
        "  Runtime command: /rich-media [status|on|off|clear-cache]".into(),
    ];
    if matches!(action, RichMediaAction::Enable) && !changed {
        lines.push("  Note: no supported terminal image protocol was detected".into());
    }
    if let Some(reloaded_sources) = reloaded_sources {
        lines.push(format!("  Reloaded active sources: {reloaded_sources}").into());
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
