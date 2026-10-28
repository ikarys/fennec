# token-usage

A small append-only SQLite ledger for LLM calls. It stores generic call
records and answers aggregate queries without knowing about any product,
session, project, pricing, network, UI, or LLM runtime.

## Public API

Ledger::open(path) -> Result<Ledger, Error>
Ledger::open_in_memory() -> Result<Ledger, Error>
Ledger::record(&self, &Call) -> Result<bool, Error>
Ledger::import(&self, impl IntoIterator<Item = Call>) -> Result<usize, Error>
Ledger::summary(&self, &Filter) -> Result<Summary, Error>
Ledger::series(&self, &Filter, GroupBy) -> Result<Vec<Row>, Error>
Call { external_id, at_ms, product, group_id, group_name, run_id, kind, model, usage, total_secs, prefill_speed, generation_speed, breakdown, savings, labels }
Usage { prompt_tokens, cached_tokens, completion_tokens }
Filter { product, group_id, run_id, kind, model, since_ms, until_ms, label }
GroupBy::Day | GroupBy::Group | GroupBy::Model | GroupBy::Kind | GroupBy::Run
Summary { calls, prompt_tokens, cached_tokens, completion_tokens, cache_hit_rate, total_secs, breakdown, savings }
Row { key, name, summary }
Error::Sqlite | Error::Json | Error::Version | Error::Io
// JSON field names are camelCase; maps default to empty.
// external_id is an idempotency key; None always inserts.
// import also fills the labels of a known external_id stored without any.

## What it does NOT do

- No pricing
- No network
- No UI
- No LLM calls
