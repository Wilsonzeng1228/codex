use super::*;
use crate::app_event::RichMediaAction;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::HistoryCell;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Arc;

fn rendered_cell_text(cell: &dyn HistoryCell) -> String {
    cell.display_lines(/*width*/ 80)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn disabling_rich_media_reflows_formula_to_text_fallback() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    app.transcript_cells = vec![Arc::new(AgentMarkdownCell::new(
        "Gain: $x^2+y^2$.".to_string(),
        Path::new("/tmp"),
    ))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let placeholder_rows =
        crate::media::MediaPlaceholderRows::try_from(4).expect("non-zero placeholder rows");
    tui.set_chat_media_capability_for_test(crate::media::ImageProtocol::Kitty, placeholder_rows);

    app.apply_rich_media_action(&mut tui, RichMediaAction::Disable)?;

    assert_eq!(tui.chat_media_placeholder_rows(), None);
    assert!(tui.pending_history_media_placements().is_empty());
    assert!(
        app.transcript_cells
            .iter()
            .map(|cell| rendered_cell_text(cell.as_ref()))
            .any(|text| text.contains("$x^2+y^2$"))
    );
    Ok(())
}

#[tokio::test]
async fn rich_media_status_card_reports_runtime_policy() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let placeholder_rows =
        crate::media::MediaPlaceholderRows::try_from(4).expect("non-zero placeholder rows");
    tui.set_chat_media_capability_for_test(
        crate::media::ImageProtocol::Iterm2Inline,
        placeholder_rows,
    );

    app.apply_rich_media_action(&mut tui, RichMediaAction::Status)?;

    let status = app
        .transcript_cells
        .last()
        .expect("status command should append one history cell");
    insta::assert_snapshot!(rendered_cell_text(status.as_ref()), @r###"
• Rich media: on
  Protocol: iTerm2 inline
  Placeholder rows: 4
  LaTeX renderer: RaTeX (ready)
  Remote images: HTTPS public addresses only
  Runtime command: /rich-media [status|on|off]
"###);
    Ok(())
}
