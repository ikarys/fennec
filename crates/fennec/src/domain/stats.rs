//! Port of openfox `MessageStats` (shared/types.ts), the subset one model
//! call gives: what it cost in tokens and how fast it ran.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageStats {
    pub model: String,
    /// Wall clock of the call, in seconds.
    pub total_time: f64,
    /// The whole prompt sent, cached part included.
    pub prefill_tokens: u64,
    /// The part of the prompt the server took from its cache.
    pub cached_prompt_tokens: u64,
    pub generation_tokens: u64,
    /// New (non-cached) prompt tokens per second, until the first token.
    pub prefill_speed: f64,
    /// Generated tokens per second, after the first token.
    pub generation_speed: f64,
}

/// Token counts of one call, as the server reports them.
pub struct Usage {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
}

impl MessageStats {
    /// Speeds come from the wall clock, so they hold for any provider:
    /// `first_token` is when the first streamed chunk arrived.
    pub fn measure(
        model: &str,
        usage: &Usage,
        first_token: Duration,
        total: Duration,
    ) -> MessageStats {
        let per_second = |tokens: u64, d: Duration| {
            let s = d.as_secs_f64();
            if s > 0.0 { tokens as f64 / s } else { 0.0 }
        };
        let new_prompt = usage.prompt_tokens.saturating_sub(usage.cached_tokens);
        MessageStats {
            model: model.into(),
            total_time: total.as_secs_f64(),
            prefill_tokens: usage.prompt_tokens,
            cached_prompt_tokens: usage.cached_tokens,
            generation_tokens: usage.completion_tokens,
            prefill_speed: per_second(new_prompt, first_token),
            generation_speed: per_second(
                usage.completion_tokens,
                total.saturating_sub(first_token),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_count_only_new_prompt_tokens_and_time_after_the_first_token() {
        let usage = Usage {
            prompt_tokens: 1200,
            cached_tokens: 1000,
            completion_tokens: 50,
        };
        let s = MessageStats::measure(
            "m",
            &usage,
            Duration::from_millis(500),
            Duration::from_secs(3),
        );
        assert_eq!(s.prefill_speed, 400.0);
        assert_eq!(s.generation_speed, 20.0);
        let zero = MessageStats::measure("m", &usage, Duration::ZERO, Duration::ZERO);
        assert_eq!((zero.prefill_speed, zero.generation_speed), (0.0, 0.0));
    }
}
