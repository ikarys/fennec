//! The context gauge of a session's header, computed here so the UI only
//! draws it (`GET /api/sessions/{id}/context`).

use crate::domain::compaction::model_view;
use crate::domain::context_parts::{PartKind, context_parts, display_label, spread};
use crate::domain::conversation::{fold_messages, frozen_prompt};
use crate::domain::event::Event;
use crate::domain::lang::Lang;
use serde::Serialize;
use serde_json::Value;
use token_usage::display::{self, tokens};

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Ok,
    Warn,
    Err,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextView {
    /// 0 to 100, the bar's width.
    pub percent: u64,
    pub tone: Tone,
    /// Where the threshold mark sits, 0 to 100; none when compaction is off.
    pub compact_mark: Option<u64>,
    /// `6 %`, or `12,3k tokens` when the window is unknown.
    pub line: String,
    /// The tooltip's first line: taken, left, total, threshold.
    pub title: String,
    pub composition: Option<CompositionView>,
}

/// What the context is made of; token figures are spread pro rata of the
/// text, so they are indicative.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionView {
    /// The stacked bar: one segment per kind, in a fixed order.
    pub kinds: Vec<KindView>,
    /// Largest first.
    pub parts: Vec<PartView>,
    pub images: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindView {
    pub kind: PartKind,
    pub label: String,
    pub tokens: String,
    /// Share of the context, 0 to 100.
    pub percent: u64,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartView {
    /// None for the row that sums the parts past `MAX_PARTS`.
    pub kind: Option<PartKind>,
    pub label: String,
    pub tokens: String,
    /// Share of the context, 0 to 100.
    pub percent: u64,
}

/// From the last `context.state` the model still sees; `window` and
/// `compact_at` are the current settings, used when no event says otherwise.
pub fn context_view(events: &[Event], window: u64, compact_at: f64, lang: Lang) -> ContextView {
    let last = model_view(events).iter().rev().find_map(|e| match **e {
        Event::ContextState {
            current_tokens,
            max_tokens,
            compact_at,
        } => Some((current_tokens, max_tokens, compact_at)),
        _ => None,
    });

    let (current, max, compact) = match last {
        Some((current, max, compact)) => (
            current,
            if max > 0 { max } else { window },
            compact.unwrap_or(compact_at),
        ),
        None => (0, window, compact_at),
    };

    let percent = if max == 0 {
        0
    } else {
        (current as f64 * 100.0 / max as f64).round().min(100.0) as u64
    };

    let threshold = if compact > 0.0 {
        compact * 100.0
    } else {
        100.0
    };
    let tone = if percent as f64 >= threshold - 5.0 {
        Tone::Err
    } else if percent as f64 >= threshold - 15.0 {
        Tone::Warn
    } else {
        Tone::Ok
    };

    let compact_mark = (compact > 0.0).then(|| (compact * 100.0).round() as u64);

    let line = if max > 0 {
        format!("{} %", percent)
    } else {
        format!("{} tokens", tokens(current, lang.locale()))
    };

    let title = if max > 0 {
        let mut title = match lang {
            Lang::En => format!(
                "{} taken · {} left · {} total",
                tokens(current, lang.locale()),
                tokens(max.saturating_sub(current), lang.locale()),
                tokens(max, lang.locale())
            ),
            Lang::Fr => format!(
                "{} pris · {} restants · {} total",
                tokens(current, lang.locale()),
                tokens(max.saturating_sub(current), lang.locale()),
                tokens(max, lang.locale())
            ),
        };
        if let Some(mark) = compact_mark {
            title.push_str(&match lang {
                Lang::En => format!(" · compaction at {mark} %"),
                Lang::Fr => format!(" · compaction à {mark} %"),
            });
        } else {
            title.push_str(lang.pick(" · compaction off", " · compaction désactivée"));
        }
        title
    } else {
        format!("{} tokens", tokens(current, lang.locale()))
    };

    let composition = (current > 0)
        .then(|| composition(events, current, lang))
        .flatten();

    ContextView {
        percent,
        tone,
        compact_mark,
        line,
        title,
        composition,
    }
}

/// What the context holds, ready to draw; none when nothing is known.
fn composition(events: &[Event], current: u64, lang: Lang) -> Option<CompositionView> {
    let (prompt, tools) = frozen_prompt(events);
    let parts = context_parts(
        prompt.unwrap_or(""),
        tools.unwrap_or(&Value::Null),
        &fold_messages(events),
    );
    if parts.parts.is_empty() {
        return None;
    }
    let shares = spread(&parts.parts, current);
    let shares: Vec<(PartKind, String, u64)> = parts
        .parts
        .into_iter()
        .zip(shares)
        .map(|(part, share)| (part.kind, part.label, share))
        .collect();
    Some(composition_view(&shares, current, parts.images, lang))
}

/// A composition ready to draw from shares already in tokens, largest first;
/// `total` is what the percents are of. Insight draws the ledger's with it too.
pub fn composition_view(
    shares: &[(PartKind, String, u64)],
    total: u64,
    images: u64,
    lang: Lang,
) -> CompositionView {
    let mut kinds: Vec<KindView> = Vec::new();
    for kind in [PartKind::Prompt, PartKind::Conversation, PartKind::Tools] {
        let share: u64 = shares
            .iter()
            .filter(|(part_kind, _, _)| *part_kind == kind)
            .map(|(_, _, share)| share)
            .sum();
        if share > 0 {
            kinds.push(KindView {
                kind,
                label: kind_label(kind, lang).into(),
                tokens: tokens(share, lang.locale()),
                percent: display::percent(share, total),
            });
        }
    }

    let mut rows: Vec<PartView> = shares
        .iter()
        .take(MAX_PARTS)
        .map(|(kind, label, share)| {
            part_view(Some(*kind), display_label(label, lang), *share, total, lang)
        })
        .collect();
    let rest = &shares[rows.len()..];
    if !rest.is_empty() {
        rows.push(part_view(
            None,
            match lang {
                Lang::En => format!("{} other parts", rest.len()),
                Lang::Fr => format!("{} autres parts", rest.len()),
            },
            rest.iter().map(|(_, _, share)| share).sum(),
            total,
            lang,
        ));
    }

    CompositionView {
        kinds,
        parts: rows,
        images: match images {
            0 => None,
            1 => Some(
                lang.pick("+ 1 image, not estimated", "+ 1 image, non estimée")
                    .to_string(),
            ),
            n => Some(match lang {
                Lang::En => format!("+ {n} images, not estimated"),
                Lang::Fr => format!("+ {n} images, non estimées"),
            }),
        },
    }
}

fn part_view(
    kind: Option<PartKind>,
    label: String,
    share: u64,
    current: u64,
    lang: Lang,
) -> PartView {
    PartView {
        kind,
        label,
        tokens: tokens(share, lang.locale()),
        percent: display::percent(share, current),
    }
}

fn kind_label(kind: PartKind, lang: Lang) -> &'static str {
    match kind {
        PartKind::Prompt => lang.pick("System prompt", "Prompt système"),
        PartKind::Conversation => "Conversation",
        PartKind::Tools => lang.pick("Tools", "Outils"),
    }
}

/// Past this many rows the popover stops being read; the rest is one row.
const MAX_PARTS: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::Role;

    fn state(current: u64, max: u64, compact_at: Option<f64>) -> Event {
        Event::ContextState {
            current_tokens: current,
            max_tokens: max,
            compact_at,
        }
    }

    #[test]
    fn no_event_uses_the_settings() {
        let view = context_view(&[], 200_000, 0.85, Lang::Fr);
        assert_eq!(view.percent, 0);
        assert_eq!(view.tone, Tone::Ok);
        assert_eq!(view.compact_mark, Some(85));
        assert_eq!(view.line, "0 %");
    }

    #[test]
    fn the_last_visible_event_wins_and_sets_the_tone() {
        let events = [state(170_000, 200_000, Some(0.9))];
        let view = context_view(&events, 100_000, 0.5, Lang::Fr);
        assert_eq!(view.percent, 85);
        assert_eq!(view.tone, Tone::Err);
        assert_eq!(view.compact_mark, Some(90));
        assert_eq!(view.line, "85 %");
    }

    #[test]
    fn compaction_off_has_no_mark_and_says_so() {
        let events = [state(50_000, 100_000, Some(0.0))];
        let view = context_view(&events, 100_000, 0.5, Lang::Fr);
        assert_eq!(view.percent, 50);
        assert_eq!(view.tone, Tone::Ok);
        assert_eq!(view.compact_mark, None);
        assert!(view.title.ends_with("compaction désactivée"));
    }

    #[test]
    fn compaction_off_has_no_mark_and_says_so_in_english() {
        let events = [state(50_000, 100_000, Some(0.0))];
        let view = context_view(&events, 100_000, 0.5, Lang::En);
        assert_eq!(view.compact_mark, None);
        assert!(view.title.ends_with("compaction off"));
    }

    #[test]
    fn the_last_event_wins() {
        let events = [
            state(10_000, 100_000, Some(0.8)),
            state(90_000, 100_000, Some(0.9)),
        ];
        let view = context_view(&events, 100_000, 0.5, Lang::Fr);
        assert_eq!(view.percent, 90);
        assert_eq!(view.compact_mark, Some(90));
    }

    #[test]
    fn an_unknown_window_shows_only_the_tokens() {
        let events = [state(500, 0, None)];
        let view = context_view(&events, 0, 0.85, Lang::Fr);
        assert_eq!(view.percent, 0);
        assert_eq!(view.line, "500 tokens");
    }

    #[test]
    fn the_composition_groups_the_parts_by_kind() {
        let events = vec![
            Event::TurnSnapshot {
                cached_system_prompt: "prompt".into(),
                cached_tools: Some(serde_json::json!([])),
                mcp_tool_names: Vec::new(),
            },
            Event::MessageStart {
                message_id: "u".into(),
                role: Role::User,
                content: Some("hello".into()),
                is_system_generated: false,
                metadata: None,
                attachments: Vec::new(),
            },
            state(100, 1000, Some(0.85)),
        ];
        let view = context_view(&events, 1000, 0.85, Lang::Fr);
        let composition = view.composition.expect("composition");
        let kinds: Vec<PartKind> = composition.kinds.iter().map(|k| k.kind).collect();
        assert_eq!(kinds, [PartKind::Prompt, PartKind::Conversation]);
        assert!(
            composition
                .parts
                .iter()
                .any(|p| p.label == "Messages humains" && p.kind == Some(PartKind::Conversation))
        );
        assert_eq!(composition.images, None);
    }
}
