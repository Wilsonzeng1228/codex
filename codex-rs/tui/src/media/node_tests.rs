use super::MediaNode;
use super::extract_media_nodes;

#[test]
fn extracts_only_markdown_images_in_source_order() {
    let markdown = concat!(
        "`![not an image](D:/ignored.png)` ",
        "![first **diagram**](https://example.org/first.png) ",
        "![second](file:///D:/course/second.png)"
    );

    assert_eq!(
        extract_media_nodes(markdown),
        vec![
            MediaNode::Image {
                source: "https://example.org/first.png".to_string(),
                alt: "first diagram".to_string(),
                ordinal: 0,
            },
            MediaNode::Image {
                source: "file:///D:/course/second.png".to_string(),
                alt: "second".to_string(),
                ordinal: 1,
            },
        ]
    );
}

#[test]
fn extracts_latex_and_images_in_source_order_without_touching_code_or_unclosed_math() {
    let markdown = concat!(
        "Before $x^2$ ",
        "![diagram](https://example.org/diagram.png) ",
        "$$\\frac{1}{s+1}$$ ",
        "`$not_math$` ",
        "\\$also_not_math$ ",
        "price $5.00 ",
        "$unclosed"
    );

    assert_eq!(
        extract_media_nodes(markdown),
        vec![
            MediaNode::Latex {
                source: "x^2".to_string(),
                display: false,
                ordinal: 0,
            },
            MediaNode::Image {
                source: "https://example.org/diagram.png".to_string(),
                alt: "diagram".to_string(),
                ordinal: 1,
            },
            MediaNode::Latex {
                source: "\\frac{1}{s+1}".to_string(),
                display: true,
                ordinal: 2,
            },
        ]
    );
}

#[test]
fn user_authored_latex_internal_looking_image_destination_stays_an_image() {
    let markdown = "![literal](codex-latex:0) then $x^2$";

    assert_eq!(
        extract_media_nodes(markdown),
        vec![
            MediaNode::Image {
                source: "codex-latex:0".to_string(),
                alt: "literal".to_string(),
                ordinal: 0,
            },
            MediaNode::Latex {
                source: "x^2".to_string(),
                display: false,
                ordinal: 1,
            },
        ]
    );
}
