"""Threaded stdlib HTTP server that fakes or proxies the OpenAI model API for fennec perf runs."""

from __future__ import annotations

import json
import secrets
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class ModelServer:
    """Serves /v1/models and /v1/chat/completions on a local free port."""

    def __init__(self, upstream: str | None, api_key: str | None, latency_s: float) -> None:
        self.upstream = upstream
        self.api_key = api_key
        self.latency_s = latency_s
        self._calls = 0
        self._busy_s = 0.0
        self._lock = threading.Lock()
        self._server: ThreadingHTTPServer | None = None
        self._thread: threading.Thread | None = None

    def start(self) -> str:
        """Start the server and return its base URL."""
        self._server = ThreadingHTTPServer(("127.0.0.1", 0), _ModelHandler)
        self._server.daemon_threads = True
        self._server.model = self  # type: ignore[attr-defined]
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()
        port = self._server.server_address[1]
        return f"http://127.0.0.1:{port}/v1"

    def stop(self) -> None:
        """Stop the server and wait briefly for its thread."""
        if self._server is not None:
            self._server.shutdown()
            self._server.server_close()
            self._server = None
        if self._thread is not None:
            self._thread.join(timeout=1.0)
            self._thread = None

    def _record_chat(self, busy_s: float) -> None:
        """Record one chat/completions call and its busy time."""
        with self._lock:
            self._calls += 1
            self._busy_s += busy_s

    def snapshot(self) -> tuple[int, float]:
        """Return thread-safe chat call count and total busy seconds."""
        with self._lock:
            return self._calls, self._busy_s


class _ModelHandler(BaseHTTPRequestHandler):
    """HTTP handler for the model server."""

    protocol_version = "HTTP/1.0"

    def log_message(self, format: str, *args: object) -> None:
        """Silence default request logging."""

    @property
    def model(self) -> ModelServer:
        """Return the owning ModelServer."""
        return self.server.model  # type: ignore[attr-defined]

    def do_GET(self) -> None:
        """Handle GET requests."""
        if self.path == "/v1/models":
            if self.model.upstream is None:
                self._send_json(200, {"data": [{"id": "perf-fake", "max_model_len": 32768}]})
            else:
                self._proxy("GET")
        else:
            self._send_json(404, {"error": "not found"})

    def do_POST(self) -> None:
        """Handle POST requests."""
        if self.path == "/v1/chat/completions":
            started = time.perf_counter()
            body = self._read_body()
            if self.model.upstream is None:
                self._fake_chat(body)
            else:
                self._proxy("POST", body)
            self.model._record_chat(time.perf_counter() - started)
        else:
            self._send_json(404, {"error": "not found"})

    def _read_body(self) -> bytes:
        """Read the request body when present."""
        length = int(self.headers.get("Content-Length", "0") or 0)
        return self.rfile.read(length) if length else b""

    def _send_json(self, status: int, payload: dict) -> None:
        """Send a JSON response and close the HTTP/1.0 connection."""
        data = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _start_sse(self) -> None:
        """Start an SSE response."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()

    def _send_sse(self, payload: dict) -> None:
        """Send one SSE data line."""
        self.wfile.write(f"data: {json.dumps(payload)}\n\n".encode())
        self.wfile.flush()

    def _finish_sse(self) -> None:
        """Send the SSE terminator."""
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()

    def _fake_chat(self, body: bytes) -> None:
        """Produce a scripted fake chat completion."""
        request = json.loads(body.decode()) if body else {}
        time.sleep(self.model.latency_s)
        self._start_sse()

        messages = request.get("messages", [])
        tools = request.get("tools", [])
        step = _tool_step(messages)
        tool_names = _tool_names(tools)

        if not tools:
            _stream_text(self, "Summary: the session read, wrote and listed files.")
        elif step == 0:
            _stream_tool(self, "read_file", {"path": "README.md"})
        elif step == 1:
            user_count = sum(1 for message in messages if message.get("role") == "user")
            _stream_tool(
                self,
                "write_file",
                {"path": f"notes/note-{user_count}.md", "content": "Perf note content. " * 108},
            )
        elif step == 2:
            _stream_tool(self, "run_command", {"command": "ls -la"})
        elif step == 3 and "submit_report" in tool_names:
            _stream_tool(
                self,
                "submit_report",
                {
                    "title": "Perf build",
                    "summary": "Wrote a note.",
                    "steps": [{"title": "Write the note", "done": True, "note": "notes written"}],
                },
            )
        else:
            _stream_text(self, "Perf answer text. " * 36)

        usage = {
            "prompt_tokens": len(json.dumps(messages, separators=(",", ":"))) // 4,
            "completion_tokens": 50,
            "prompt_tokens_details": {"cached_tokens": 0},
        }
        self._send_sse({"choices": [], "usage": usage})
        self._finish_sse()

    def _proxy(self, method: str, body: bytes | None = None) -> None:
        """Forward a request to the upstream model server."""
        # The upstream base already ends in /v1, like the path this server answers.
        url = self.model.upstream.rstrip("/") + self.path.removeprefix("/v1")
        headers: dict[str, str] = {}
        if self.model.api_key:
            headers["Authorization"] = f"Bearer {self.model.api_key}"
        if method == "POST":
            headers["Content-Type"] = "application/json"

        req = urllib.request.Request(
            url,
            data=body if method == "POST" else None,
            headers=headers,
            method=method,
        )
        try:
            with urllib.request.urlopen(req, timeout=600) as resp:
                self.send_response(resp.status)
                self.send_header("Content-Type", resp.headers.get("Content-Type", "application/octet-stream"))
                self.end_headers()
                while True:
                    chunk = resp.read1(4096)
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                    self.wfile.flush()
        except urllib.error.HTTPError as exc:
            self._send_json(exc.code, {"error": exc.read().decode(errors="replace")})
        except urllib.error.URLError as exc:
            self._send_json(502, {"error": str(exc.reason)})


def _tool_step(messages: list[dict]) -> int:
    """Count tool messages after the last user message."""
    last_user = -1
    for index, message in enumerate(messages):
        if message.get("role") == "user":
            last_user = index
    if last_user == -1:
        return 0
    return sum(1 for message in messages[last_user + 1 :] if message.get("role") == "tool")


def _tool_names(tools: list[dict]) -> set[str]:
    """Extract function names from the offered tools."""
    names: set[str] = set()
    for tool in tools:
        if not isinstance(tool, dict):
            continue
        function = tool.get("function")
        if isinstance(function, dict) and "name" in function:
            names.add(str(function["name"]))
    return names


def _stream_text(handler: _ModelHandler, text: str) -> None:
    """Stream text content chunks and a stop finish chunk."""
    chunk_size = max(1, len(text) // 10)
    for start in range(0, len(text), chunk_size):
        handler._send_sse({"choices": [{"delta": {"content": text[start : start + chunk_size]}}]})
    handler._send_sse({"choices": [{"delta": {}, "finish_reason": "stop"}]})


def _stream_tool(handler: _ModelHandler, name: str, args: dict) -> None:
    """Stream one tool call chunk and a tool_calls finish chunk."""
    call_id = "call-" + secrets.token_hex(6)
    handler._send_sse(
        {
            "choices": [
                {
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": call_id,
                                "function": {"name": name, "arguments": json.dumps(args)},
                            }
                        ]
                    },
                    "finish_reason": "tool_calls",
                }
            ]
        }
    )
