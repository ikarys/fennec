# fennec performance — latest run

Produced by `just perf` in an isolated temporary workspace. The fake model is scripted with a fixed latency so numbers compare across commits on the same machine. History is stored in `history.jsonl`, one JSON line per run.

- date: 2026-11-10T14:16:33Z
- commit: 9e53fb7c
- machine: IKS-DESKTOP
- cpu: AMD Ryzen 9 9950X3D 16-Core Processor
- mode: fake

| metric | value | previous | delta |
| --- | --- | --- | --- |
| startup_ms | 42.5 | — | — |
| rss_rest_mb | 8.6 | — | — |
| rss_peak_mb | 15.6 | — | — |
| rss_end_mb | 12.5 | — | — |
| cpu_s | 0.6 | — | — |
| db_mb | 5.5 | — | — |
| turn_wall_p50_ms | 265.0 | — | — |
| turn_wall_p95_ms | 306.0 | — | — |
| kernel_overhead_p50_ms | 60.9 | — | — |
| kernel_overhead_p95_ms | 62.0 | — | — |
| compaction_ms | 133.6 | — | — |
| workflow_ms | 367.1 | — | — |
| model_calls | 46 | — | — |
| page.home.p50_ms | 1.4 | — | — |
| page.home.p95_ms | 1.7 | — | — |
| page.projects.p50_ms | 6.4 | — | — |
| page.projects.p95_ms | 6.8 | — | — |
| page.insight7.p50_ms | 1.5 | — | — |
| page.insight7.p95_ms | 2.2 | — | — |
| page.insight30.p50_ms | 1.5 | — | — |
| page.insight30.p95_ms | 1.7 | — | — |
| page.backlog.p50_ms | 1.4 | — | — |
| page.backlog.p95_ms | 1.7 | — | — |
| page.dashboard.p50_ms | 3.2 | — | — |
| page.dashboard.p95_ms | 3.9 | — | — |
| page.session.p50_ms | 4.3 | — | — |
| page.session.p95_ms | 5.3 | — | — |
| page.events.p50_ms | 2.6 | — | — |
| page.events.p95_ms | 3.1 | — | — |
| page.thread.p50_ms | 4.0 | — | — |
| page.thread.p95_ms | 4.5 | — | — |
| page.context.p50_ms | 1.7 | — | — |
| page.context.p95_ms | 2.0 | — | — |
| page.usage.p50_ms | 1.4 | — | — |
| page.usage.p95_ms | 1.7 | — | — |
