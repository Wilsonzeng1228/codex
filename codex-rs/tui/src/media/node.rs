use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;

use super::rewrite_latex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MediaNode {
    Image {
        source: String,
        alt: String,
        ordinal: usize,
    },
    Latex {
        source: String,
        display: bool,
        ordinal: usize,
    },
}

pub(crate) fn extract_media_nodes(markdown: &str) -> Vec<MediaNode> {
    let latex = rewrite_latex(markdown);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let mut nodes = Vec::new();
    let mut image: Option<(String, String)> = None;
    for event in Parser::new_ext(&latex.markdown, options) {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                image = Some((dest_url.into_string(), String::new()));
            }
            Event::End(TagEnd::Image) => {
                if let Some((source, alt)) = image.take() {
                    let ordinal = nodes.len();
                    if let Some(spec) = latex.spec_for_destination(&source) {
                        nodes.push(MediaNode::Latex {
                            source: spec.source.clone(),
                            display: spec.display,
                            ordinal,
                        });
                    } else {
                        nodes.push(MediaNode::Image {
                            source,
                            alt,
                            ordinal,
                        });
                    }
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, alt)) = image.as_mut() {
                    alt.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, alt)) = image.as_mut()
                    && !alt.ends_with(' ')
                {
                    alt.push(' ');
                }
            }
            _ => {}
        }
    }

    nodes
}
