"""Terminal and markdown reporting for fennec performance runs."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path


def previous(history: Path, machine: str, mode: str) -> dict | None:
    """Return the last history record for this machine and mode, if any."""
    if not history.exists():
        return None
    try:
        lines = history.read_text().splitlines()
    except OSError:
        return None

    for line in reversed(lines):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if record.get("machine") == machine and record.get("mode") == mode:
            return record
    return None


def table(record: dict, prev: dict | None) -> list[tuple[str, str, str, str]]:
    """Build formatted table rows: metric, value, previous, delta."""
    metrics = record.get("metrics", {})
    prev_metrics = prev.get("metrics", {}) if prev else {}
    rows: list[tuple[str, str, str, str]] = []

    for metric, value in metrics.items():
        prev_value = prev_metrics.get(metric) if prev else None
        delta = _delta_percent(metric, value, prev_value)
        if prev is None:
            delta_str = "—"
        elif delta is None:
            delta_str = ""
        else:
            delta_str = f"{delta:+.1f} %"

        rows.append(
            (
                metric,
                _format_value(metric, value),
                _format_value(metric, prev_value) if prev_value is not None else "—",
                delta_str,
            )
        )
    return rows


def print_terminal(record: dict, prev: dict | None) -> None:
    """Print an aligned terminal table with optional ANSI colors."""
    header = (
        f"fennec perf — {record.get('mode')} — {record.get('commit')} "
        f"on {record.get('machine')} ({record.get('date')})"
    )
    rows = table(record, prev)
    headers = ("metric", "value", "previous", "delta")
    widths = [
        max(len(str(header)), *(len(row[index]) for row in rows)) if rows else len(str(header))
        for index, header in enumerate(headers)
    ]

    print(header)
    print(" ".join(str(header).ljust(widths[index]) for index, header in enumerate(headers)))
    print(" ".join("-" * width for width in widths))

    use_color = sys.stdout.isatty() and not os.environ.get("NO_COLOR")
    metrics = record.get("metrics", {})
    prev_metrics = prev.get("metrics", {}) if prev else {}
    regressions = 0

    for metric, value, previous, delta in rows:
        current = metrics.get(metric)
        prev_value = prev_metrics.get(metric) if prev else None
        delta_percent = _delta_percent(metric, current, prev_value)
        # Sub-2 ms timings swing by more than 20 % from noise alone.
        noise = (
            metric.endswith("_ms")
            and current is not None
            and prev_value is not None
            and float(current) < 2.0
            and float(prev_value) < 2.0
        )
        colored_delta = delta
        if delta_percent is not None and not noise:
            if delta_percent > 20:
                regressions += 1
                colored_delta = f"\033[31m{delta}\033[0m" if use_color else delta
            elif delta_percent < -20 and use_color:
                colored_delta = f"\033[32m{delta}\033[0m"

        print(
            " ".join(
                (
                    metric.ljust(widths[0]),
                    value.ljust(widths[1]),
                    previous.ljust(widths[2]),
                    colored_delta,
                )
            )
        )

    print(f"Regressions over +20%: {regressions}")


def write_markdown(path: Path, record: dict, prev: dict | None) -> None:
    """Write the latest run as a markdown report."""
    path.parent.mkdir(parents=True, exist_ok=True)

    lines = [
        "# fennec performance — latest run",
        "",
        "Produced by `just perf` in an isolated temporary workspace. The fake model is scripted "
        "with a fixed latency so numbers compare across commits on the same machine. History is "
        "stored in `history.jsonl`, one JSON line per run.",
        "",
        f"- date: {record.get('date')}",
        f"- commit: {record.get('commit')}",
        f"- machine: {record.get('machine')}",
        f"- cpu: {record.get('cpu')}",
        f"- mode: {record.get('mode')}",
        "",
        "| metric | value | previous | delta |",
        "| --- | --- | --- | --- |",
    ]

    for metric, value, previous, delta in table(record, prev):
        lines.append(f"| {metric} | {value} | {previous} | {delta} |")

    path.write_text("\n".join(lines) + "\n")


def append_history(path: Path, record: dict) -> None:
    """Append one JSON line to the history file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")


def _format_value(metric: str, value: float | int | None) -> str:
    """Format a metric value for display."""
    if value is None:
        return "—"
    if metric == "model_calls":
        return str(int(value))
    return f"{float(value):.1f}"


def _delta_percent(
    metric: str,
    current: float | int | None,
    previous_value: float | int | None,
) -> float | None:
    """Return percent change from previous to current, or None when unavailable."""
    if current is None or previous_value is None:
        return None
    prev = float(previous_value)
    if prev == 0:
        return None
    return (float(current) - prev) / abs(prev) * 100.0
