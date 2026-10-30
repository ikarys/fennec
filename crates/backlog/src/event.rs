use serde::{Deserialize, Serialize};

/// Append-only events that describe a backlog's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum BacklogEvent {
    #[serde(rename = "idea.added", rename_all = "camelCase")]
    Added { idea_id: String, text: String },
    #[serde(rename = "idea.framing", rename_all = "camelCase")]
    Framing { idea_id: String, session_id: String },
    #[serde(rename = "idea.ready", rename_all = "camelCase")]
    Ready { idea_id: String, plan: String },
    #[serde(rename = "idea.started", rename_all = "camelCase")]
    Started {
        idea_id: String,
        #[serde(default)]
        autonomous: bool,
    },
    #[serde(rename = "idea.dropped", rename_all = "camelCase")]
    Dropped { idea_id: String },
    #[serde(rename = "idea.completed", rename_all = "camelCase")]
    Completed { idea_id: String },
    #[serde(rename = "idea.halted", rename_all = "camelCase")]
    Halted { idea_id: String, reason: String },
    #[serde(rename = "idea.reordered", rename_all = "camelCase")]
    Reordered { ids: Vec<String> },
    #[serde(rename = "idea.queue_started", rename_all = "camelCase")]
    QueueStarted { autonomous: bool },
    #[serde(rename = "idea.queue_finished", rename_all = "camelCase")]
    QueueFinished { reason: String },
    #[serde(rename = "idea.report_read", rename_all = "camelCase")]
    ReportRead {},
}

impl BacklogEvent {
    /// The wire name used in the `type` field.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Added { .. } => "idea.added",
            Self::Framing { .. } => "idea.framing",
            Self::Ready { .. } => "idea.ready",
            Self::Started { .. } => "idea.started",
            Self::Dropped { .. } => "idea.dropped",
            Self::Completed { .. } => "idea.completed",
            Self::Halted { .. } => "idea.halted",
            Self::Reordered { .. } => "idea.reordered",
            Self::QueueStarted { .. } => "idea.queue_started",
            Self::QueueFinished { .. } => "idea.queue_finished",
            Self::ReportRead {} => "idea.report_read",
        }
    }

    /// The idea this event changes, if it is about one idea.
    pub fn idea_id(&self) -> Option<&str> {
        match self {
            Self::Added { idea_id, .. }
            | Self::Framing { idea_id, .. }
            | Self::Ready { idea_id, .. }
            | Self::Started { idea_id, .. }
            | Self::Dropped { idea_id }
            | Self::Completed { idea_id }
            | Self::Halted { idea_id, .. } => Some(idea_id),
            Self::Reordered { .. }
            | Self::QueueStarted { .. }
            | Self::QueueFinished { .. }
            | Self::ReportRead {} => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn added(id: &str, text: &str) -> BacklogEvent {
        BacklogEvent::Added {
            idea_id: id.into(),
            text: text.into(),
        }
    }

    fn framing(id: &str, session_id: &str) -> BacklogEvent {
        BacklogEvent::Framing {
            idea_id: id.into(),
            session_id: session_id.into(),
        }
    }

    fn ready(id: &str, plan: &str) -> BacklogEvent {
        BacklogEvent::Ready {
            idea_id: id.into(),
            plan: plan.into(),
        }
    }

    fn started(id: &str, autonomous: bool) -> BacklogEvent {
        BacklogEvent::Started {
            idea_id: id.into(),
            autonomous,
        }
    }

    fn dropped(id: &str) -> BacklogEvent {
        BacklogEvent::Dropped { idea_id: id.into() }
    }

    fn completed(id: &str) -> BacklogEvent {
        BacklogEvent::Completed { idea_id: id.into() }
    }

    fn halted(id: &str, reason: &str) -> BacklogEvent {
        BacklogEvent::Halted {
            idea_id: id.into(),
            reason: reason.into(),
        }
    }

    fn reordered(ids: &[&str]) -> BacklogEvent {
        BacklogEvent::Reordered {
            ids: ids.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    fn queue_started(autonomous: bool) -> BacklogEvent {
        BacklogEvent::QueueStarted { autonomous }
    }

    fn queue_finished(reason: &str) -> BacklogEvent {
        BacklogEvent::QueueFinished {
            reason: reason.into(),
        }
    }

    fn report_read() -> BacklogEvent {
        BacklogEvent::ReportRead {}
    }

    #[test]
    fn kind_returns_wire_names() {
        assert_eq!(added("i1", "t").kind(), "idea.added");
        assert_eq!(framing("i1", "s1").kind(), "idea.framing");
        assert_eq!(ready("i1", "p").kind(), "idea.ready");
        assert_eq!(started("i1", false).kind(), "idea.started");
        assert_eq!(dropped("i1").kind(), "idea.dropped");
        assert_eq!(completed("i1").kind(), "idea.completed");
        assert_eq!(halted("i1", "r").kind(), "idea.halted");
        assert_eq!(reordered(&["i1"]).kind(), "idea.reordered");
        assert_eq!(queue_started(true).kind(), "idea.queue_started");
        assert_eq!(queue_finished("done").kind(), "idea.queue_finished");
        assert_eq!(report_read().kind(), "idea.report_read");
    }

    #[test]
    fn idea_id_is_none_for_queue_events() {
        assert_eq!(added("i1", "t").idea_id(), Some("i1"));
        assert_eq!(framing("i1", "s1").idea_id(), Some("i1"));
        assert_eq!(ready("i1", "p").idea_id(), Some("i1"));
        assert_eq!(started("i1", false).idea_id(), Some("i1"));
        assert_eq!(dropped("i1").idea_id(), Some("i1"));
        assert_eq!(completed("i1").idea_id(), Some("i1"));
        assert_eq!(halted("i1", "r").idea_id(), Some("i1"));
        assert_eq!(reordered(&["i1"]).idea_id(), None);
        assert_eq!(queue_started(true).idea_id(), None);
        assert_eq!(queue_finished("done").idea_id(), None);
        assert_eq!(report_read().idea_id(), None);
    }

    #[test]
    fn idea_event_serde_round_trip() {
        let event = added("i1", "t");

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"type":"idea.added","data":{"ideaId":"i1","text":"t"}}"#
        );

        let decoded: BacklogEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn started_event_serde_wire() {
        let event = started("i1", true);

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"type":"idea.started","data":{"ideaId":"i1","autonomous":true}}"#
        );

        let decoded: BacklogEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn started_event_old_log_without_autonomous_deserializes_false() {
        let json = r#"{"type":"idea.started","data":{"ideaId":"i1"}}"#;

        let decoded: BacklogEvent = serde_json::from_str(json).unwrap();
        assert_eq!(decoded, started("i1", false));
    }

    #[test]
    fn halted_event_serde_wire() {
        let event = halted("i1", "r");

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"type":"idea.halted","data":{"ideaId":"i1","reason":"r"}}"#
        );

        let decoded: BacklogEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn completed_event_serde_wire() {
        let event = completed("i1");

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"type":"idea.completed","data":{"ideaId":"i1"}}"#);

        let decoded: BacklogEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn report_read_serializes_with_empty_data() {
        let event = report_read();

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"type":"idea.report_read","data":{}}"#);

        let decoded: BacklogEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }
}
