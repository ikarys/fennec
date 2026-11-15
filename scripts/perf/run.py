"""Run the fennec performance harness and write history/latest reports."""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import socket
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import NoReturn

import model as model_module
import report as report_module

REPO_ROOT = Path(__file__).resolve().parents[2]
# A real model ends a turn in `waiting_for_user` when it asks a question (a normal,
# finished turn); only the scripted fake model guarantees it never happens.
FAILED_STATES = {"error", "truncated"}
FAKE_FAILED_STATES = {"waiting_for_user", "error", "truncated"}


def failed_states(mode: str) -> set[str]:
    """The session states that fail a turn, by mode."""
    return FAKE_FAILED_STATES if mode == "fake" else FAILED_STATES


def main() -> int:
    """Run the perf harness and return a process exit code."""
    args = parse_args()
    root = Path(tempfile.mkdtemp(prefix="fennec-perf-"))
    home = root / "home"
    data = root / "data"
    project = root / "project"
    home.mkdir()
    data.mkdir()
    project.mkdir()

    server: subprocess.Popen[bytes] | None = None
    fake_model: model_module.ModelServer | None = None
    log_path = root / "server.log"

    try:
        init_project_repo(project)
        fake_model = build_model(args.mode)
        model_url = fake_model.start()
        model_id = resolve_model_id(args.mode, model_url)
        port = free_port()
        server = start_fennec(port, home, data, model_url, model_id, log_path)
        startup_s = wait_for_home(port, server, log_path)

        time.sleep(1.0)
        rss_rest_mb = proc_field_mb(server.pid, "VmRSS")

        turn_timeout_s = 120.0 if args.mode == "fake" else 900.0
        failed = failed_states(args.mode)
        project_id = create_project(port, project)
        first_sid = ""
        turn_walls_ms: list[float] = []
        kernel_overheads_ms: list[float] = []

        for index in range(args.sessions):
            before = fake_model.snapshot()
            sid, wall_s = start_session(port, project_id, f"Read the README, write a note and list the files (session {index})", turn_timeout_s, failed)
            after = fake_model.snapshot()
            record_turn(wall_s, before, after, turn_walls_ms, kernel_overheads_ms)
            if index == 0:
                first_sid = sid

            for _ in range(args.turns - 1):
                before = fake_model.snapshot()
                done_before = chat_done_count(port, sid)
                started = time.perf_counter()
                status, _ = http_request(port, "POST", f"/api/sessions/{sid}/messages", {"content": "Continue"}, 30.0)
                if status not in (200, 202):
                    fail(f"message POST returned {status}")
                wall_s = wait_turn(port, sid, turn_timeout_s, started, done_before, failed)
                after = fake_model.snapshot()
                record_turn(wall_s, before, after, turn_walls_ms, kernel_overheads_ms)

        compaction_ms = run_compaction(port, first_sid, turn_timeout_s, failed)
        workflow_ms = run_workflow(port, project_id, turn_timeout_s)
        page_metrics = measure_pages(port, project_id, first_sid, args.samples)

        record = build_record(
            args.mode,
            startup_s,
            rss_rest_mb,
            server.pid,
            data,
            turn_walls_ms,
            kernel_overheads_ms,
            compaction_ms,
            workflow_ms,
            page_metrics,
            fake_model.snapshot()[0],
        )

        history_path = REPO_ROOT / "docs" / "devs" / "perf" / "history.jsonl"
        prev = report_module.previous(history_path, record["machine"], record["mode"])
        report_module.print_terminal(record, prev)

        if not args.no_record:
            report_module.append_history(history_path, record)
            report_module.write_markdown(REPO_ROOT / "docs" / "devs" / "perf" / "latest.md", record, prev)

        return 0
    finally:
        if server is not None:
            stop_fennec(server)
        if fake_model is not None:
            fake_model.stop()
        shutil.rmtree(root, ignore_errors=True)


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description="Run fennec performance harness.")
    parser.add_argument("--mode", choices=("fake", "real"), required=True)
    parser.add_argument("--sessions", type=int, default=3)
    parser.add_argument("--turns", type=int, default=3)
    parser.add_argument("--samples", type=int, default=50)
    parser.add_argument("--no-record", action="store_true")
    args = parser.parse_args()
    args.sessions = max(1, args.sessions)
    args.turns = max(1, args.turns)
    args.samples = max(1, args.samples)
    return args


def init_project_repo(project: Path) -> None:
    """Create a throwaway git project with a README and small sources."""
    src = project / "src"
    src.mkdir()
    lines = ["Fennec perf README"]
    lines.extend(f"Line {index}: deterministic content for the perf repo." for index in range(29))
    (project / "README.md").write_text("\n".join(lines) + "\n")
    for index in range(5):
        (src / f"file{index}.txt").write_text(f"small perf file {index}\n")

    run_git(project, "init", "-q")
    run_git(project, "add", "-A")
    run_git(
        project,
        "-c",
        "user.name=perf",
        "-c",
        "user.email=perf@localhost",
        "commit",
        "-qm",
        "init",
    )


def run_git(project: Path, *args: str) -> None:
    """Run a git command inside the project directory."""
    subprocess.run(["git", *args], cwd=project, check=True)


def build_model(mode: str) -> model_module.ModelServer:
    """Create the fake or proxy model server."""
    if mode == "fake":
        return model_module.ModelServer(None, None, 0.05)
    upstream = os.environ.get("FENNEC_LLM_URL", "http://127.0.0.1:8000/v1")
    api_key = os.environ.get("FENNEC_API_KEY")
    return model_module.ModelServer(upstream, api_key, 0.0)


def resolve_model_id(mode: str, model_url: str) -> str:
    """Return the model id to pass to fennec."""
    configured = os.environ.get("FENNEC_MODEL")
    if configured:
        return configured
    if mode == "fake":
        return "perf-fake"
    return first_model_id(model_url)


def first_model_id(base_url: str) -> str:
    """Fetch the first model id from an OpenAI-compatible /models endpoint."""
    try:
        with urllib.request.urlopen(base_url + "/models", timeout=5.0) as resp:
            payload = json.load(resp)
    except Exception as exc:  # noqa: BLE001 - report one clear message
        fail(f"cannot reach model at {base_url}: {exc}")

    for item in payload.get("data", []):
        if isinstance(item, dict) and item.get("id"):
            return str(item["id"])
    fail("model server returned no model ids")


def free_port() -> int:
    """Return a free localhost TCP port."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def start_fennec(
    port: int,
    home: Path,
    data: Path,
    model_url: str,
    model_id: str,
    log_path: Path,
) -> subprocess.Popen[bytes]:
    """Start fennec with isolated environment variables."""
    binary = REPO_ROOT / "target" / "release" / "fennec"
    if not binary.exists():
        fail("run `cargo build --release -p fennec` first")

    env = {key: value for key, value in os.environ.items() if not key.startswith("FENNEC_")}
    env.update(
        {
            "HOME": str(home),
            "FENNEC_DB": str(data / "fennec.db"),
            "FENNEC_LLM_URL": model_url,
            "FENNEC_MODEL": model_id,
            "FENNEC_COMPACT_AT": "0",
            "GIT_AUTHOR_NAME": "perf",
            "GIT_AUTHOR_EMAIL": "perf@localhost",
            "GIT_COMMITTER_NAME": "perf",
            "GIT_COMMITTER_EMAIL": "perf@localhost",
        }
    )

    log_file = log_path.open("wb")
    return subprocess.Popen(
        [str(binary), "--port", str(port)],
        cwd=REPO_ROOT,
        env=env,
        stdout=log_file,
        stderr=subprocess.STDOUT,
    )


def wait_for_home(port: int, server: subprocess.Popen[bytes], log_path: Path) -> float:
    """Poll until GET /api/home returns 200 and return startup time."""
    started = time.perf_counter()
    deadline = started + 30.0

    while time.perf_counter() < deadline:
        if server.poll() is not None:
            print_log_tail(log_path, 30)
            fail("fennec exited during startup")
        status, _ = http_request(port, "GET", "/api/home", None, 1.0)
        if status == 200:
            return time.perf_counter() - started
        time.sleep(0.02)

    print_log_tail(log_path, 30)
    fail("fennec did not answer /api/home within 30s")


def stop_fennec(server: subprocess.Popen[bytes]) -> None:
    """Terminate fennec, waiting up to 10 seconds before killing it."""
    server.terminate()
    try:
        server.wait(timeout=10)
    except subprocess.TimeoutExpired:
        server.kill()
        server.wait()


def http_request(
    port: int,
    method: str,
    path: str,
    payload: dict | None = None,
    timeout: float = 30.0,
) -> tuple[int, dict]:
    """Send an HTTP request to fennec and return status plus parsed JSON."""
    url = f"http://127.0.0.1:{port}{path}"
    headers = {"Host": f"127.0.0.1:{port}"}
    body = None

    if payload is not None:
        body = json.dumps(payload).encode()
        headers["Content-Type"] = "application/json"
        headers["Origin"] = f"http://127.0.0.1:{port}"

    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
            status = resp.status
    except urllib.error.HTTPError as exc:
        raw = exc.read()
        status = exc.code
    except urllib.error.URLError as exc:
        return 599, {"error": str(exc.reason)}

    try:
        return status, json.loads(raw) if raw else {}
    except json.JSONDecodeError:
        return status, {"error": "invalid JSON"}


def create_project(port: int, project: Path) -> str:
    """Create a fennec project and return its id."""
    status, body = http_request(port, "POST", "/api/projects", {"workdir": str(project)})
    if status not in (200, 201):
        fail(f"project creation returned {status}: {body.get('error', 'unknown error')}")
    project_id = body.get("id")
    if not project_id:
        fail("project creation response has no id")
    return str(project_id)


def start_session(port: int, project_id: str, prompt: str, timeout_s: float, failed: set[str]) -> tuple[str, float]:
    """Create a normal session, wait for its first turn, and return id and wall time."""
    started = time.perf_counter()
    status, body = http_request(
        port,
        "POST",
        "/api/sessions",
        {"projectId": project_id, "prompt": prompt, "mode": "builder"},
        30.0,
    )
    if status not in (200, 201, 202):
        fail(f"session creation returned {status}: {body.get('error', 'unknown error')}")
    sid = body.get("id")
    if not sid:
        fail("session creation response has no id")
    wall_s = wait_turn(port, str(sid), timeout_s, started, 0, failed)
    return str(sid), wall_s


def record_turn(
    wall_s: float,
    before: tuple[int, float],
    after: tuple[int, float],
    turn_walls_ms: list[float],
    kernel_overheads_ms: list[float],
) -> None:
    """Record wall time and kernel overhead for one turn."""
    busy_delta_s = after[1] - before[1]
    turn_walls_ms.append(wall_s * 1000.0)
    kernel_overheads_ms.append(max(0.0, wall_s - busy_delta_s) * 1000.0)


def run_compaction(port: int, sid: str, timeout_s: float, failed: set[str]) -> float:
    """Compact the first session and return elapsed milliseconds."""
    done_before = chat_done_count(port, sid)
    started = time.perf_counter()
    status, body = http_request(port, "POST", f"/api/sessions/{sid}/compact", {}, 30.0)
    if status not in (200, 202):
        fail(f"compaction returned {status}: {body.get('error', 'unknown error')}")
    return wait_turn(port, sid, timeout_s, started, done_before, failed) * 1000.0


def run_workflow(port: int, project_id: str, timeout_s: float) -> float:
    """Start a quick workflow and return elapsed milliseconds when finished."""
    started = time.perf_counter()
    status, body = http_request(
        port,
        "POST",
        "/api/sessions",
        {
            "projectId": project_id,
            "prompt": "Write a note about the project",
            "workflowId": "quick",
            "worktree": False,
        },
        30.0,
    )
    if status not in (200, 201, 202):
        fail(f"workflow session creation returned {status}: {body.get('error', 'unknown error')}")
    sid = body.get("id")
    if not sid:
        fail("workflow session creation response has no id")

    deadline = started + timeout_s
    answered = 0
    while time.perf_counter() < deadline:
        status, view = http_request(port, "GET", f"/api/sessions/{sid}/workflow/run/view", None, 5.0)
        if status == 200 and isinstance(view, dict):
            finished = view.get("finished")
            if isinstance(finished, dict) and finished.get("outcome"):
                outcome = finished["outcome"]
                if outcome != "done":
                    fail(f"workflow finished with outcome {outcome!r}")
                return (time.perf_counter() - started) * 1000.0
        # A real model may end an agent turn with a question; answer it (bounded) so the run finishes.
        if answered < 3:
            status, session = http_request(port, "GET", f"/api/sessions/{sid}", None, 5.0)
            if status == 200 and session.get("state") == "waiting_for_user":
                status, body = http_request(port, "POST", f"/api/sessions/{sid}/messages", {"content": "Continue"}, 30.0)
                if status in (200, 202):
                    answered += 1
        time.sleep(0.05)

    fail("workflow did not finish in time")


def measure_pages(port: int, project_id: str, sid: str, samples: int) -> dict[str, float]:
    """Measure sequential page GETs and return p50/p95 metrics."""
    pages = [
        ("home", "/api/home"),
        ("projects", "/api/projects"),
        ("insight7", "/api/insight?days=7"),
        ("insight30", "/api/insight?days=30"),
        ("backlog", "/api/backlog"),
        ("dashboard", f"/api/projects/{project_id}/dashboard"),
        ("session", f"/api/sessions/{sid}"),
        ("events", f"/api/sessions/{sid}/events?limit=200"),
        ("thread", f"/api/sessions/{sid}/thread"),
        ("context", f"/api/sessions/{sid}/context"),
        ("usage", f"/api/sessions/{sid}/usage"),
    ]

    metrics: dict[str, float] = {}
    for short, path in pages:
        times_ms: list[float] = []
        for _ in range(samples):
            started = time.perf_counter()
            status, body = http_request(port, "GET", path, None, 30.0)
            if status != 200:
                fail(f"page {path} returned {status}: {body.get('error', 'unknown error')}")
            times_ms.append((time.perf_counter() - started) * 1000.0)

        metrics[f"page.{short}.p50_ms"] = percentile_50(times_ms)
        metrics[f"page.{short}.p95_ms"] = percentile_95(times_ms)
    return metrics


def percentile_50(values: list[float]) -> float:
    """Return the median of a non-empty list."""
    if len(values) == 1:
        return values[0]
    return statistics.median(values)


def percentile_95(values: list[float]) -> float:
    """Return the 95th percentile using statistics.quantiles(n=20)."""
    if len(values) == 1:
        return values[0]
    return statistics.quantiles(values, n=20)[18]


def proc_field_mb(pid: int, field: str) -> float | None:
    """Read a /proc status field in kB and return MB, or None off Linux."""
    path = Path(f"/proc/{pid}/status")
    if not path.exists():
        return None
    try:
        for line in path.read_text().splitlines():
            if line.startswith(field + ":"):
                return int(line.split()[1]) / 1024.0
    except (OSError, ValueError, IndexError):
        return None
    return None


def proc_cpu_seconds(pid: int) -> float | None:
    """Return process CPU seconds from /proc/<pid>/stat, or None off Linux."""
    path = Path(f"/proc/{pid}/stat")
    if not path.exists():
        return None
    try:
        content = path.read_text()
        fields = content.rsplit(")", 1)[1].split()
        tick = os.sysconf("SC_CLK_TCK")
        return (int(fields[11]) + int(fields[12])) / tick
    except (OSError, ValueError, IndexError):
        return None


def db_size_mb(data: Path) -> float:
    """Return total size of all files under data in MB."""
    total = sum(file.stat().st_size for file in data.rglob("*") if file.is_file())
    return total / (1024 * 1024)


def cpu_name() -> str:
    """Return the CPU model name from /proc/cpuinfo, falling back to platform."""
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor()


def git_commit() -> str:
    """Return short git commit, marked dirty when the worktree is dirty."""
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except subprocess.CalledProcessError:
        return "unknown"

    try:
        dirty = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except subprocess.CalledProcessError:
        dirty = ""

    return f"{out}-dirty" if dirty else out


def build_record(
    mode: str,
    startup_s: float,
    rss_rest_mb: float | None,
    pid: int,
    data: Path,
    turn_walls_ms: list[float],
    kernel_overheads_ms: list[float],
    compaction_ms: float,
    workflow_ms: float,
    page_metrics: dict[str, float],
    model_calls: int,
) -> dict:
    """Build the record dict used by report.py."""
    rss_peak_mb = proc_field_mb(pid, "VmHWM")
    rss_end_mb = proc_field_mb(pid, "VmRSS")
    cpu_s = proc_cpu_seconds(pid)

    metrics: dict[str, float | int | None] = {
        "startup_ms": startup_s * 1000.0,
        "rss_rest_mb": rss_rest_mb,
        "rss_peak_mb": rss_peak_mb,
        "rss_end_mb": rss_end_mb,
        "cpu_s": cpu_s,
        "db_mb": db_size_mb(data),
        "turn_wall_p50_ms": percentile_50(turn_walls_ms),
        "turn_wall_p95_ms": percentile_95(turn_walls_ms),
        "kernel_overhead_p50_ms": percentile_50(kernel_overheads_ms),
        "kernel_overhead_p95_ms": percentile_95(kernel_overheads_ms),
        "compaction_ms": compaction_ms,
        "workflow_ms": workflow_ms,
        "model_calls": model_calls,
    }
    metrics.update(page_metrics)

    return {
        "date": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "commit": git_commit(),
        "machine": platform.node(),
        "cpu": cpu_name(),
        "mode": mode,
        "metrics": metrics,
    }


def chat_dones(port: int, sid: str) -> list[dict]:
    """The session's `chat.done` payloads: one per finished turn, compaction included."""
    status, events = http_request(port, "GET", f"/api/sessions/{sid}/events?limit=10000", None, 5.0)
    if status != 200 or not isinstance(events, list):
        return []
    return [
        item["event"].get("data") or {}
        for item in events
        if item.get("event", {}).get("type") == "chat.done"
    ]


def chat_done_count(port: int, sid: str) -> int:
    """How many turns of the session have ended."""
    return len(chat_dones(port, sid))


def wait_turn(port: int, sid: str, timeout_s: float, started: float, done_before: int, failed: set[str]) -> float:
    """Poll until the turn started at `started` has ended and return its duration in seconds.

    The state alone is ambiguous right after a POST (the previous turn's "complete"), so the
    turn has ended once a new `chat.done` is in the log and the session no longer runs. The
    duration is the kernel's own clock (`durationMs`) when the event has it, else the wall
    clock seen from here (compaction-only turns).
    """
    deadline = started + timeout_s
    while time.perf_counter() < deadline:
        status, body = http_request(port, "GET", f"/api/sessions/{sid}", None, 5.0)
        state = body.get("state", "error") if status == 200 else "error"
        dones = chat_dones(port, sid) if state != "running" else []
        if len(dones) > done_before:
            if state in failed:
                fail(f"session {sid} ended in state {state}")
            duration_ms = dones[-1].get("durationMs")
            if isinstance(duration_ms, (int, float)):
                return duration_ms / 1000.0
            return time.perf_counter() - started
        time.sleep(0.02)
    fail(f"session {sid} did not finish within {timeout_s:.0f}s")


def print_log_tail(log_path: Path, lines: int) -> None:
    """Print the last lines of a server log."""
    if not log_path.exists():
        return
    try:
        tail = log_path.read_text().splitlines()[-lines:]
    except OSError:
        return
    if tail:
        print("".join(line + "\n" for line in tail), file=sys.stderr)


def fail(message: str) -> NoReturn:
    """Print an error and exit."""
    print(message, file=sys.stderr)
    raise SystemExit(1)


if __name__ == "__main__":
    sys.exit(main())
