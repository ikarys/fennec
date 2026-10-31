//! The images a session's model received, and which of them are still in its
//! view: the UI's Vision block is computed here, not in the browser, because
//! the log is lazy and the image window is a server rule.

use super::compaction::model_view;
use super::event::Event;
use serde::Serialize;

/// At most this many images go to the model: inference servers cap the media of one request
/// (a local vision model refused 7 screenshots).
const MAX_IMAGES: usize = 6;
/// The window moves by this many images, so the earlier messages change only once every
/// STEP images (each change re-reads the prompt from the first dropped image, see AGENTS.md,
/// the prompt only grows); and the last MAX_IMAGES - STEP + 1 = 4 images are always kept,
/// so an image just read is never dropped by the read itself.
const STEP: usize = 3;
/// With `n` images in the model's view, the index of the first one sent: 0 up to MAX_IMAGES,
/// then ceil((n - MAX_IMAGES) / STEP) * STEP.
pub fn first_kept_image(n: usize) -> usize {
    if n <= MAX_IMAGES {
        0
    } else {
        (n - MAX_IMAGES).div_ceil(STEP) * STEP
    }
}

/// An image the model received: a tool result or an attachment of the human.
/// `anchor_id` is the DOM id of the thread's tool card or attachment
/// (`web/src/lib/vision.ts` `toolAnchorId`/`attachmentAnchorId`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeenImage {
    pub label: String,
    pub anchor_id: String,
    pub src: String,
    pub in_context: bool,
}

/// The images one event sends, in the order `fold_messages` sends them:
/// (label, anchor id, data URL).
fn images(event: &Event) -> Vec<(&str, String, &str)> {
    match event {
        Event::MessageStart { attachments, .. } => attachments
            .iter()
            .map(|a| {
                (
                    a.filename.as_str(),
                    format!("attachment-{}", a.id),
                    a.data.as_str(),
                )
            })
            .collect(),
        Event::ToolResult {
            tool_call_id,
            result,
            ..
        } => match &result.metadata {
            Some(metadata) if metadata.mime_type.starts_with("image/") => vec![(
                metadata.path.as_str(),
                format!("tool-{tool_call_id}"),
                metadata.data_url.as_str(),
            )],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Every image of the whole log, in log order, that the model received;
/// `in_context` is true only while the model's current view still sends it.
pub fn seen_images(events: &[Event]) -> Vec<SeenImage> {
    // The images the model is sent, as (event, index within the event): the
    // view borrows from `events`, so an event is matched by its address.
    let view: Vec<(*const Event, usize)> = model_view(events)
        .into_iter()
        .flat_map(|event| (0..images(event).len()).map(move |i| (event as *const Event, i)))
        .collect();
    let sent = &view[first_kept_image(view.len())..];
    events
        .iter()
        .flat_map(|event| {
            images(event)
                .into_iter()
                .enumerate()
                .map(move |(i, (label, anchor_id, src))| SeenImage {
                    label: label.to_string(),
                    anchor_id,
                    src: src.to_string(),
                    in_context: sent.contains(&(event as *const Event, i)),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::attachment::Attachment;
    use crate::domain::event::{Role, ToolResult, ToolResultMetadata};

    fn attachment_message(attachment_id: &str, filename: &str, data: &str) -> Event {
        Event::MessageStart {
            message_id: "u".into(),
            role: Role::User,
            content: Some("look".into()),
            is_system_generated: false,
            metadata: None,
            attachments: vec![Attachment {
                id: attachment_id.into(),
                filename: filename.into(),
                mime_type: "image/png".into(),
                size: 1,
                data: data.into(),
            }],
        }
    }

    fn image_tool_result(tool_call_id: &str, path: &str, data_url: &str) -> Event {
        Event::ToolResult {
            message_id: "a".into(),
            tool_call_id: tool_call_id.into(),
            result: ToolResult::image(
                "img".into(),
                ToolResultMetadata {
                    mime_type: "image/png".into(),
                    data_url: data_url.into(),
                    path: path.into(),
                    size: 1,
                },
            ),
        }
    }

    #[test]
    fn first_kept_image_keeps_at_most_six_and_at_least_four() {
        assert_eq!(first_kept_image(0), 0);
        assert_eq!(first_kept_image(4), 0);
        assert_eq!(first_kept_image(6), 0);
        assert_eq!(first_kept_image(7), 3);
        assert_eq!(first_kept_image(9), 3);
        assert_eq!(first_kept_image(10), 6);
        assert_eq!(first_kept_image(12), 6);
        assert_eq!(first_kept_image(13), 9);
        for n in 1..=40 {
            let kept = n - first_kept_image(n);
            assert!((1..=6).contains(&kept));
            if n > 6 {
                assert!(kept >= 4);
            }
        }
    }

    #[test]
    fn seen_images_lists_an_attachment_and_an_image_tool_result_in_log_order() {
        let events = vec![
            attachment_message("a1", "a.png", "data:image/png;base64,AAAA"),
            image_tool_result("c1", "b.png", "data:image/png;base64,BBBB"),
        ];
        let seen = seen_images(&events);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].label, "a.png");
        assert_eq!(seen[0].anchor_id, "attachment-a1");
        assert_eq!(seen[0].src, "data:image/png;base64,AAAA");
        assert!(seen[0].in_context);
        assert_eq!(seen[1].label, "b.png");
        assert_eq!(seen[1].anchor_id, "tool-c1");
        assert_eq!(seen[1].src, "data:image/png;base64,BBBB");
        assert!(seen[1].in_context);
    }

    #[test]
    fn seen_images_drops_the_images_the_window_no_longer_holds() {
        let events: Vec<Event> = (0..10)
            .map(|i| {
                image_tool_result(
                    &format!("c{i}"),
                    &format!("img{i}.png"),
                    &format!("data:image/png;base64,{i}"),
                )
            })
            .collect();
        let seen = seen_images(&events);
        assert_eq!(seen.len(), 10);
        for image in &seen[..6] {
            assert!(!image.in_context);
        }
        for image in &seen[6..] {
            assert!(image.in_context);
        }
    }

    #[test]
    fn seen_images_marks_images_before_a_compaction_out_of_context() {
        let events = vec![
            attachment_message("a1", "a.png", "data:image/png;base64,AAAA"),
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "s".into(),
            },
            image_tool_result("c1", "b.png", "data:image/png;base64,BBBB"),
        ];
        let seen = seen_images(&events);
        assert_eq!(seen.len(), 2);
        assert!(!seen[0].in_context);
        assert!(seen[1].in_context);
    }
}
