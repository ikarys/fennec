# fennec performance — latest run

Produced by `just perf` in an isolated temporary workspace. The fake model is scripted with a fixed latency so numbers compare across commits on the same machine. History is stored in `history.jsonl`, one JSON line per run.

- date: 2026-11-12T20:30:22Z
- commit: 180482f9-dirty
- machine: IKS-DESKTOP
- cpu: AMD Ryzen 9 9950X3D 16-Core Processor
- mode: real

| metric | value | previous | delta |
| --- | --- | --- | --- |
| startup_ms | 23.6 | — | — |
| rss_rest_mb | 8.8 | — | — |
| rss_peak_mb | 15.0 | — | — |
| rss_end_mb | 14.5 | — | — |
| cpu_s | 4.8 | — | — |
| db_mb | 5.5 | — | — |
| turn_wall_p50_ms | 5464.0 | — | — |
| turn_wall_p95_ms | 9202.0 | — | — |
| kernel_overhead_p50_ms | 62.3 | — | — |
| kernel_overhead_p95_ms | 231.0 | — | — |
| compaction_ms | 5432.7 | — | — |
| workflow_ms | 6530.5 | — | — |
| model_calls | 40 | — | — |
| page.home.p50_ms | 1.2 | — | — |
| page.home.p95_ms | 1.4 | — | — |
| page.projects.p50_ms | 5.2 | — | — |
| page.projects.p95_ms | 5.8 | — | — |
| page.insight7.p50_ms | 1.2 | — | — |
| page.insight7.p95_ms | 1.6 | — | — |
| page.insight30.p50_ms | 1.2 | — | — |
| page.insight30.p95_ms | 1.7 | — | — |
| page.backlog.p50_ms | 1.1 | — | — |
| page.backlog.p95_ms | 1.3 | — | — |
| page.dashboard.p50_ms | 2.6 | — | — |
| page.dashboard.p95_ms | 3.1 | — | — |
| page.session.p50_ms | 4.2 | — | — |
| page.session.p95_ms | 5.0 | — | — |
| page.events.p50_ms | 2.0 | — | — |
| page.events.p95_ms | 2.4 | — | — |
| page.thread.p50_ms | 3.2 | — | — |
| page.thread.p95_ms | 3.7 | — | — |
| page.context.p50_ms | 1.4 | — | — |
| page.context.p95_ms | 1.9 | — | — |
| page.usage.p50_ms | 1.2 | — | — |
| page.usage.p95_ms | 1.8 | — | — |
