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
