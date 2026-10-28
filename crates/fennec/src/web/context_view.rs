//! The context gauge of a session's header, computed here so the UI only
//! draws it (`GET /api/sessions/{id}/context`).

use crate::domain::compaction::model_view;
use crate::domain::event::Event;
use serde::Serialize;
use token_usage::display::tokens;

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
    /// `12,3k / 200,0k · 6 %`, or `12,3k tokens` when the window is unknown.
    pub line: String,
    /// The tooltip.
    pub title: String,
}

/// From the last `context.state` the model still sees; `window` and
/// `compact_at` are the current settings, used when no event says otherwise.
pub fn context_view(events: &[Event], window: u64, compact_at: f64) -> ContextView {
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
        format!("{} / {} · {} %", tokens(current), tokens(max), percent)
    } else {
        format!("{} tokens", tokens(current))
    };

    let title = if max > 0 {
        let mut title = format!(
            "{} pris · {} restants · {} total",
            tokens(current),
            tokens(max.saturating_sub(current)),
            tokens(max)
        );
        if let Some(mark) = compact_mark {
            title.push_str(&format!(" · compaction à {mark} %"));
        } else {
            title.push_str(" · compaction désactivée");
        }
        title
    } else {
        format!("{} tokens", tokens(current))
    };

    ContextView {
        percent,
        tone,
        compact_mark,
        line,
        title,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(current: u64, max: u64, compact_at: Option<f64>) -> Event {
        Event::ContextState {
            current_tokens: current,
            max_tokens: max,
            compact_at,
        }
    }

    #[test]
    fn no_event_uses_the_settings() {
        let view = context_view(&[], 200_000, 0.85);
        assert_eq!(view.percent, 0);
        assert_eq!(view.tone, Tone::Ok);
        assert_eq!(view.compact_mark, Some(85));
        assert_eq!(view.line, "0 / 200,0k · 0 %");
    }

    #[test]
    fn the_last_visible_event_wins_and_sets_the_tone() {
        let events = [state(170_000, 200_000, Some(0.9))];
        let view = context_view(&events, 100_000, 0.5);
        assert_eq!(view.percent, 85);
        assert_eq!(view.tone, Tone::Err);
        assert_eq!(view.compact_mark, Some(90));
        assert_eq!(view.line, "170,0k / 200,0k · 85 %");
    }

    #[test]
    fn compaction_off_has_no_mark_and_says_so() {
        let events = [state(50_000, 100_000, Some(0.0))];
        let view = context_view(&events, 100_000, 0.5);
        assert_eq!(view.percent, 50);
        assert_eq!(view.tone, Tone::Ok);
        assert_eq!(view.compact_mark, None);
        assert!(view.title.ends_with("compaction désactivée"));
    }

    #[test]
    fn the_last_event_wins() {
        let events = [
            state(10_000, 100_000, Some(0.8)),
            state(90_000, 100_000, Some(0.9)),
        ];
        let view = context_view(&events, 100_000, 0.5);
        assert_eq!(view.percent, 90);
        assert_eq!(view.compact_mark, Some(90));
    }

    #[test]
    fn an_unknown_window_shows_only_the_tokens() {
        let events = [state(500, 0, None)];
        let view = context_view(&events, 0, 0.85);
        assert_eq!(view.percent, 0);
        assert_eq!(view.line, "500 tokens");
    }
}
