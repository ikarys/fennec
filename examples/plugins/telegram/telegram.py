#!/usr/bin/env python3
"""fennec plugin: a Telegram bot. Stdlib only.

Told of an event (stdin), it sends the message to your chat, with buttons for
the decision offered: commit or drop proven work, validate a "done when".
Replying to a question's message answers it.

Run by fennec with FENNEC_URL and FENNEC_TOKEN (`serve`), it long-polls
Telegram and relays your buttons and replies to fennec. fennec accepts them
only from the senders `allow` lists in plugin.json and writes each in the
task's Journal.

Setup: create a bot with @BotFather, put its token and your chat id in
bot.json, and your Telegram user id (a number) in plugin.json's "allow".
"""

import json
import os
import sys
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
BOT = json.load(open(os.path.join(HERE, "bot.json")))
# Which task each message is about, so a reply or a button finds it.
PENDING = os.path.join(HERE, "pending.json")


def telegram(method, **params):
    req = urllib.request.Request(
        f"https://api.telegram.org/bot{BOT['token']}/{method}",
        data=json.dumps(params).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=70) as r:
        return json.load(r)["result"]


def pending():
    try:
        return json.load(open(PENDING))
    except (OSError, ValueError):
        return {}


def remember(message_id, task, status):
    p = pending()
    p[str(message_id)] = {"task": task, "status": status}
    # Only the latest 200: older messages are long decided.
    p = dict(list(p.items())[-200:])
    tmp = PENDING + ".tmp"
    json.dump(p, open(tmp, "w"))
    os.replace(tmp, PENDING)


def notify():
    event = json.load(sys.stdin)
    task, status = event.get("task", ""), event.get("status", "")
    text = event.get("message", "")
    if event.get("url"):
        text += "\n" + event["url"]
    buttons = []
    if status == "ready":
        buttons = [("Valider", f"land:{task}"), ("Abandonner", f"drop:{task}")]
    elif status == "question":
        buttons = [("Valider", f"answer:{task}"), ("Abandonner", f"drop:{task}")]
        text += "\n(réponds à ce message pour corriger)"
    elif status in ("talk", "blocked"):
        text += "\n(réponds à ce message)"
    params = {"chat_id": BOT["chat"], "text": text}
    if buttons:
        params["reply_markup"] = {
            "inline_keyboard": [[{"text": t, "callback_data": d} for t, d in buttons]]
        }
    sent = telegram("sendMessage", **params)
    if task:
        remember(sent["message_id"], task, status)


def fennec(path, body):
    req = urllib.request.Request(
        os.environ["FENNEC_URL"] + path,
        data=json.dumps(body).encode(),
        headers={
            "Content-Type": "application/json",
            "Authorization": "Bearer " + os.environ["FENNEC_TOKEN"],
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=30):
            return "c'est fait"
    except urllib.error.HTTPError as e:
        why = json.load(e).get("error", str(e))
        if why == "jeton inconnu":
            # fennec restarted or turned this plugin off: another runs now.
            sys.exit(0)
        return why
    except urllib.error.URLError:
        sys.exit(0)


def serve():
    parent, offset = os.getppid(), 0
    while os.getppid() == parent:
        try:
            updates = telegram("getUpdates", offset=offset, timeout=50)
        except (urllib.error.URLError, TimeoutError) as e:
            print(f"telegram: {e}", file=sys.stderr)
            continue
        for u in updates:
            offset = u["update_id"] + 1
            if "callback_query" in u:
                q = u["callback_query"]
                action, _, task = q.get("data", "").partition(":")
                body = {"id": task, "from": q["from"]["id"]}
                if action == "answer":
                    body.update(accept=True, correction="")
                said = fennec(f"/api/{action}", body)
                telegram("answerCallbackQuery", callback_query_id=q["id"], text=said)
            elif "reply_to_message" in u.get("message", {}):
                m = u["message"]
                about = pending().get(str(m["reply_to_message"]["message_id"]))
                if not about:
                    continue
                path = "/api/reply" if about["status"] == "blocked" else "/api/say"
                said = fennec(path, {"id": about["task"], "from": m["from"]["id"], "text": m.get("text", "")})
                telegram("sendMessage", chat_id=m["chat"]["id"], text=said,
                         reply_to_message_id=m["message_id"])


if __name__ == "__main__":
    serve() if os.environ.get("FENNEC_TOKEN") else notify()
