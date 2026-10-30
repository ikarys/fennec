use serde::{Deserialize, Serialize};

/// A file or image a human pins to an idea, kept with it as a `data:` URL so
/// the build and the framing can hand it to the agent without a second store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaAttachment {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    /// A `data:` URL, the form both the board and the model API take.
    pub data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idea_attachment_round_trips() {
        let attachment = IdeaAttachment {
            filename: "spec.md".into(),
            mime_type: "text/markdown".into(),
            size: 4,
            data: "data:text/markdown;base64,Y2hh".into(),
        };

        let json = serde_json::to_string(&attachment).unwrap();
        assert_eq!(
            json,
            r#"{"filename":"spec.md","mimeType":"text/markdown","size":4,"data":"data:text/markdown;base64,Y2hh"}"#
        );

        let decoded: IdeaAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, attachment);
    }
}
