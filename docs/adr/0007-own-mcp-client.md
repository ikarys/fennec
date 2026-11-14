---
status: accepted
date: "2026-11-13"
---

# fennec keeps its own synchronous MCP client rather than rmcp

## Context and problem statement

fennec talks to MCP servers through a synchronous client on ureq, now the `mcp-lite` crate on crates.io (then `crates/fennec/src/mcp.rs`)
(stdio and streamable HTTP; `initialize`, `tools/list`, `tools/call`). `rmcp`, the official Rust
SDK, covers the whole protocol and is maintained by others. Reuse comes first, so rmcp was tried
on a branch behind the same synchronous API (a current-thread tokio runtime per connection).

## Measurements

Release binary, and medians over 15 calls against one remote HTTP MCP server, two alternating
rounds each (2026-11-13):

| | Own client | rmcp |
| --- | --- | --- |
| Release binary | 10.6 MB | 23.4 MB |
| Crates in the lockfile | 119 | 221 |
| `initialize` | 250 ms | 250 ms |
| `tools/list` | 172 ms | 230 ms |
| `tools/call` | 634 ms | 682 ms |
| Peak RSS of the test process | 8.7 MB | 11.6 MB |

The tool catalog and the meta tool definitions came out byte-identical: the prompt cache is not
concerned either way.

## Decision

Keep the own client. rmcp brings tokio and reqwest with hyper, a second HTTP stack next to ureq,
which llm-providers-connector needs anyway: nothing is removed, the binary doubles (rule 8). What
rmcp adds over the own client is the server-initiated half of MCP (elicitation, sampling,
`roots/list`, `tools/list_changed`, progress) plus resources and prompts. fennec uses none of it:
its human already decides through its own gates, and its MCP use is tools.

The two gaps the comparison exposed are fixed in the own client: `tools/list` follows
`nextCursor` (bounded), and non-text content parts are rendered as markers instead of dropped.

## Revisit when

fennec needs a message the server initiates (elicitation, `tools/list_changed`), or MCP
resources. That needs a reader per connection; at that point rmcp is weighed again against
writing it.
