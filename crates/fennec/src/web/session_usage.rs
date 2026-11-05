//! What the Tokens panel shows, computed here so the UI only draws it:
//! a session's totals from the usage ledger (`GET /api/sessions/{id}/usage`).

use crate::domain::event::Event;
use crate::domain::lang::Lang;
use crate::domain::pricing::Pricing;
use serde::Serialize;
use std::collections::HashMap;
use token_usage::{CallRecord, LatestCall, Summary, display};

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUsageView {
    pub calls: u64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
    /// Share of the prompts the server took from its cache, 0 to 100.
    pub cache_percent: u64,
    /// Estimated cost at each model's price, "—" when no model has one: indicative.
    pub cost: String,
    /// How the cost was reached, or why there is none (models without a price).
    pub cost_detail: String,
    /// The cost of all the prompt tokens (the cached ones included, as `prompt_tokens`),
    /// none when no model has a price.
    pub input_cost: Option<String>,
    /// The share of the cost of cached tokens, none when no model has a price.
    pub cached_cost: Option<String>,
    /// The share of the cost of generated tokens, none when no model has a price.
    pub output_cost: Option<String>,
    /// Generated tokens per second of the last call; none before any call.
    pub last_generation_speed: Option<f64>,
}

impl SessionUsageView {
    pub fn new(
        summary: &Summary,
        latest: Option<&LatestCall>,
        pricing: &Pricing,
        lang: Lang,
    ) -> SessionUsageView {
        SessionUsageView {
            calls: summary.calls,
            prompt_tokens: summary.prompt_tokens,
            cached_tokens: summary.cached_tokens,
            completion_tokens: summary.completion_tokens,
            cache_percent: (summary.cache_hit_rate * 100.0).round() as u64,
            cost: pricing.value(lang.locale()),
            cost_detail: pricing.detail(lang),
            input_cost: pricing.part(pricing.cost.input + pricing.cost.cached, lang.locale()),
            cached_cost: pricing.part(pricing.cost.cached, lang.locale()),
            output_cost: pricing.part(pricing.cost.output, lang.locale()),
            last_generation_speed: latest.map(|call| call.generation_speed),
        }
    }
}

/// What sits under one answer, and, on the last call of a turn, the recap line.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallView {
    pub message_id: String,
    /// The model that answered: the thread names it, not today's choice.
    pub model: String,
    /// `↑ 12,3k · cache 82 % · ↓ 345 · 4,2 s`
    pub line: String,
    /// The same figures in full, with the speeds.
    pub detail: String,
    /// `4,2 s · ↓ 345 tokens · 82 tok/s`, on a turn's last call only.
    pub turn_recap: Option<String>,
}

/// One view per call of `events` the ledger knows, in log order. The recap
/// of a turn sits under its last call: the turn's wall clock (`chat.done`,
/// tools included: the speed the human feels) and the tokens its calls
/// generated, from the ledger.
pub fn call_views(events: &[Event], calls: &[CallRecord], lang: Lang) -> Vec<CallView> {
    let by_message: HashMap<&str, &CallRecord> = calls
        .iter()
        .filter_map(|c| Some((c.labels.get("messageId")?.as_str(), c)))
        .collect();
    let mut views: Vec<CallView> = Vec::new();
    // Generated tokens since the last `chat.done`, and the view of the turn's last call.
    let mut turn_tokens = 0u64;
    let mut last: Option<usize> = None;
    for event in events {
        match event {
            Event::MessageDone {
                message_id,
                stats: Some(_),
                partial: false,
            } => {
                let Some(call) = by_message.get(message_id.as_str()) else {
                    continue;
                };
                turn_tokens += call.usage.completion_tokens;
                last = Some(views.len());
                views.push(view(message_id, call, lang));
            }
            Event::ChatDone { duration_ms, .. } => {
                if let (Some(ms), Some(i)) = (duration_ms, last)
                    && *ms > 0
                    && turn_tokens > 0
                {
                    let secs = *ms as f64 / 1000.0;
                    views[i].turn_recap = Some(format!(
                        "{} · ↓ {} tokens · {}",
                        display::duration(secs, lang.locale()),
                        display::tokens(turn_tokens, lang.locale()),
                        display::speed(turn_tokens as f64 / secs)
                    ));
                }
                turn_tokens = 0;
                last = None;
            }
            _ => {}
        }
    }
    views
}

fn view(message_id: &str, call: &CallRecord, lang: Lang) -> CallView {
    let usage = &call.usage;
    let fresh = usage.prompt_tokens.saturating_sub(usage.cached_tokens);
    CallView {
        message_id: message_id.to_string(),
        model: call.model.clone(),
        line: format!(
            "↑ {} · cache {} % · ↓ {} · {}",
            display::tokens(usage.prompt_tokens, lang.locale()),
            display::percent(usage.cached_tokens, usage.prompt_tokens),
            display::tokens(usage.completion_tokens, lang.locale()),
            display::duration(call.total_secs, lang.locale())
        ),
        detail: match lang {
            Lang::En => format!(
                "Input {} tokens, {} cached, {fresh} new at {} · output {} tokens at {}",
                usage.prompt_tokens,
                usage.cached_tokens,
                display::speed(call.prefill_speed),
                usage.completion_tokens,
                display::speed(call.generation_speed)
            ),
            Lang::Fr => format!(
                "Entrée {} tokens dont {} en cache, {fresh} nouveaux à {} · sortie {} tokens à {}",
                usage.prompt_tokens,
                usage.cached_tokens,
                display::speed(call.prefill_speed),
                usage.completion_tokens,
                display::speed(call.generation_speed)
            ),
        },
        turn_recap: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::DoneReason;
    use crate::domain::stats::MessageStats;
    use token_usage::{Filter, Ledger};

    #[test]
    fn an_empty_ledger_gives_zeroes_and_no_speed() {
        let ledger = Ledger::open_in_memory().unwrap();
        let summary = ledger.summary(&Filter::default()).unwrap();
        let pricing = Pricing {
            cost: llm_providers::Cost::default(),
            priced: 0,
            unpriced: vec![],
        };
        let view = SessionUsageView::new(&summary, None, &pricing, Lang::Fr);
        assert_eq!(view.calls, 0);
        assert_eq!(view.cache_percent, 0);
        assert_eq!(view.last_generation_speed, None);
        assert_eq!(view.cost, "—");
        assert_eq!(view.input_cost, None);
    }

    #[test]
    fn the_cache_share_is_a_rounded_percentage() {
        let summary = Summary {
            calls: 2,
            prompt_tokens: 300,
            cached_tokens: 200,
            completion_tokens: 10,
            cache_hit_rate: 200.0 / 300.0,
            lost_cache_calls: 0,
            lost_cache_tokens: 0,
            total_secs: 1.0,
            breakdown: Default::default(),
            savings: Default::default(),
        };
        let latest = LatestCall {
            at_ms: 1,
            model: "m".into(),
            prefill_speed: 5.0,
            generation_speed: 42.5,
        };
        let pricing = Pricing {
            cost: llm_providers::Cost::default(),
            priced: 0,
            unpriced: vec![],
        };
        let view = SessionUsageView::new(&summary, Some(&latest), &pricing, Lang::Fr);
        assert_eq!(view.cache_percent, 67);
        assert_eq!(view.last_generation_speed, Some(42.5));
        assert_eq!(view.prompt_tokens, 300);
        assert_eq!(view.cost, "—");
    }

    fn record(message: &str, completion: u64) -> CallRecord {
        CallRecord {
            at_ms: 0,
            model: "m".into(),
            kind: "turn".into(),
            run_id: "r".into(),
            usage: token_usage::Usage {
                prompt_tokens: 1000,
                cached_tokens: 800,
                completion_tokens: completion,
            },
            total_secs: 2.0,
            prefill_speed: 100.0,
            generation_speed: 20.0,
            labels: [("messageId".to_string(), message.to_string())].into(),
        }
    }

    fn done(id: &str) -> Event {
        Event::MessageDone {
            message_id: id.into(),
            stats: Some(MessageStats {
                model: "m".into(),
                total_time: 0.0,
                prefill_tokens: 0,
                cached_prompt_tokens: 0,
                generation_tokens: 0,
                prefill_speed: 0.0,
                generation_speed: 0.0,
            }),
            partial: false,
        }
    }

    fn chat_done(duration_ms: Option<u64>) -> Event {
        Event::ChatDone {
            message_id: "x".into(),
            reason: DoneReason::Complete,
            duration_ms,
        }
    }

    #[test]
    fn a_turn_recap_uses_the_turn_wall_clock_on_its_last_call() {
        let events = [
            done("a"),
            done("b"),
            chat_done(Some(8000)),
            done("c"),
            chat_done(None),
        ];
        let calls = [record("a", 100), record("b", 100), record("c", 10)];
        let views = call_views(&events, &calls, Lang::Fr);
        assert_eq!(views[0].turn_recap, None);
        // 200 tokens over the 8 s the human waited, tools included.
        assert_eq!(
            views[1].turn_recap.as_deref(),
            Some("8,0 s · ↓ 200 tokens · 25 tok/s")
        );
        assert_eq!(views[0].line, "↑ 1,0k · cache 80 % · ↓ 100 · 2,0 s");
        assert_eq!(views[2].turn_recap, None, "older logs have no duration");
    }

    #[test]
    fn a_message_the_ledger_does_not_know_has_no_view() {
        let mut call = record("a", 1);
        call.labels.remove("messageId");
        assert!(call_views(&[done("a"), chat_done(Some(1000))], &[call], Lang::Fr).is_empty());
    }

    #[test]
    fn a_call_view_detail_is_english_when_the_language_is_english() {
        let call = record("a", 100);
        let views = call_views(&[done("a")], &[call], Lang::En);
        assert_eq!(
            views[0].detail,
            "Input 1000 tokens, 800 cached, 200 new at 100 tok/s · output 100 tokens at 20 tok/s"
        );
    }
}
