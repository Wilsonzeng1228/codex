use std::ops::Range;

use pulldown_cmark_latex::Event;
use pulldown_cmark_latex::Options;
use pulldown_cmark_latex::Parser;

const LATEX_DESTINATION_PREFIX: &str = "codex-latex:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LatexSpec {
    pub(crate) destination: String,
    pub(crate) source: String,
    pub(crate) full_source: String,
    pub(crate) display: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct LatexRewrite {
    pub(crate) markdown: String,
    pub(crate) specs: Vec<LatexSpec>,
}

impl LatexRewrite {
    pub(crate) fn spec_for_destination(&self, destination: &str) -> Option<&LatexSpec> {
        self.specs
            .iter()
            .find(|spec| spec.destination == destination)
    }
}

/// Rewrites parsed math events to private Markdown image nodes so the existing
/// Markdown layout engine can place formulas without recognizing dollar signs itself.
pub(crate) fn rewrite_latex(markdown: &str) -> LatexRewrite {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_MATH);

    let mut matches = Vec::<(Range<usize>, String, bool)>::new();
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        let (source, display) = match event {
            Event::InlineMath(source) => (source.into_string(), false),
            Event::DisplayMath(source) => (source.into_string(), true),
            _ => continue,
        };
        if source.is_empty() {
            continue;
        }
        let source_range = math_source_range(markdown, range, display, &source);
        matches.push((source_range, source, display));
    }

    if matches.is_empty() {
        return LatexRewrite {
            markdown: markdown.to_string(),
            specs: Vec::new(),
        };
    }

    let mut rewritten = String::with_capacity(markdown.len());
    let mut specs = Vec::with_capacity(matches.len());
    let mut cursor = 0;
    let rewrite_token = format!(
        "{:016x}{:016x}",
        rand::random::<u64>(),
        rand::random::<u64>()
    );
    for (range, source, display) in matches {
        if range.start < cursor || range.end > markdown.len() {
            continue;
        }
        rewritten.push_str(&markdown[cursor..range.start]);
        let index = specs.len();
        rewritten.push_str("![](");
        let destination = format!("{LATEX_DESTINATION_PREFIX}{rewrite_token}:{index}");
        rewritten.push_str(&destination);
        rewritten.push(')');
        specs.push(LatexSpec {
            destination,
            source,
            full_source: markdown[range.clone()].to_string(),
            display,
        });
        cursor = range.end;
    }
    rewritten.push_str(&markdown[cursor..]);

    LatexRewrite {
        markdown: rewritten,
        specs,
    }
}

fn math_source_range(
    markdown: &str,
    content: Range<usize>,
    display: bool,
    source: &str,
) -> Range<usize> {
    let delimiter = if display { "$$" } else { "$" };
    let delimiter_len = delimiter.len();
    let window_start = content.start.saturating_sub(delimiter_len);
    let window_end = content
        .end
        .saturating_add(delimiter_len)
        .min(markdown.len());
    let expected = format!("{delimiter}{source}{delimiter}");
    if let Some(offset) = markdown
        .get(window_start..window_end)
        .and_then(|window| window.find(&expected))
    {
        let start = window_start + offset;
        return start..start + expected.len();
    }
    content
}
