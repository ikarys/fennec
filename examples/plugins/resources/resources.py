# fennec's own resources in the status bar: RAM and CPU, as measured by fennec.
import json
import sys


def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "init":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"wantedEvents": [], "resources": True}})
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "usage",
              "content": {"elements": [{"kind": "text", "text": "measuring…"}]}}})
    elif msg.get("method") == "resources":
        params = msg["params"]
        ram = f"{params['rssBytes'] / (1024 * 1024):.1f} MiB"
        cpu = params.get("cpuPercent")
        if cpu is None:
            cpu_value = "…"
            tone = "none"
        else:
            cpu_value = f"{cpu:.1f} %"
            if cpu >= 90:
                tone = "danger"
            elif cpu >= 50:
                tone = "warning"
            else:
                tone = "none"
        send({"jsonrpc": "2.0", "method": "block.update", "params": {
              "block": "usage", "content": {"elements": [
                {"kind": "figure", "label": "RAM", "value": ram, "tone": "none"},
                {"kind": "figure", "label": "CPU", "value": cpu_value, "tone": tone},
              ]}}})
