#!/usr/bin/python3
"""Fake provider: a task source that speaks provider protocol v0 and fails on demand (ts-0108-dq).

    fake_provider.py [mode ...]

It reads one JSON-RPC request for each line of stdin and writes one line for each reply, the way a provider written
with the SDK does. It answers `initialize`, `list`, `get`, `apply`, `preview` and `watch` from fixed data, so a test
can tell the answers apart by what it asks. It exits when stdin closes. Each mode is one word on the command line,
with `=` and a value when it takes one:

    exit-after=N            exit with code 3 after N replies to requests other than `initialize`
    crash-on=METHOD         exit with code 4 on reading a request for METHOD, with no reply
    crash-once=METHOD:PATH  the same, but only while the file PATH does not exist: it makes the file as it exits
    hang-on=METHOD          read a request for METHOD and answer nothing for 30 seconds
    garbage-on=METHOD       answer a request for METHOD with a line that is not JSON
    unknown=METHOD          answer a request for METHOD with the JSON-RPC error "method not found"
    down-file=PATH          exit with code 5 at once, before it reads anything, while the file PATH exists
    bad-initialize          answer `initialize` with an op named `move`, which a provider may not declare
    no-preview              declare `preview: false` in the capabilities
    events-after=METHOD     write the six `watch` notifications after the reply to METHOD
    events-before=METHOD    write the six `watch` notifications before the reply to METHOD
    noise                   write a line to stderr for each request
    ignore-eof              when stdin closes, wait for 30 seconds instead of exiting

`get` answers by its `id`: `boom` is a JSON-RPC error, `missing` is no task, `echo` is a task whose `fields` hold what
this process saw (the names of its environment variables, its working folder, its pid, its arguments and what
`initialize` gave it), and any other id is a plain task.

`apply` and `preview` answer by their `op`: `ok`, `conflict` or `refused`.
"""

import json
import os
import sys
import time

HANG_SECONDS = 30
REFUSED_MESSAGE = "illegal transition"
BOOM_MESSAGE = "disk on fire"
NOT_FOUND = -32601
FAILED = -32000

MODES = {}
for word in sys.argv[1:]:
    name, _, value = word.partition("=")
    MODES[name] = value

if "down-file" in MODES and os.path.exists(MODES["down-file"]):
    sys.exit(5)

STATE = {"init": None, "replies": 0}


def say(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


def state(status="todo", category="ready"):
    return {"status": status, "category": category}


def task(id="fake-1", fields=None):
    return {
        "key": "fake:" + id.split("-")[-1],
        "id": id,
        "title": "A fake task",
        "state": state(),
        "version": "v1",
        "fields": fields or {},
    }


def effect():
    return {"key": "fake:1", "version": "v2", "from": state(), "to": state("doing", "active")}


def initialize_result():
    ops = [{"name": "move"}] if "bad-initialize" in MODES else [{"name": "setStatus"}]
    return {
        "scope": "global",
        "idPattern": "^fake-[0-9]+$",
        "shortIdPattern": "^[0-9]+$",
        "statuses": [{"name": "todo", "category": "ready"}, {"name": "doing", "category": "active"}],
        "ops": ops,
        "capabilities": {"watch": True, "preview": "no-preview" not in MODES},
    }


def echo_fields():
    return {
        "env": sorted(os.environ.keys()),
        "cwd": os.getcwd(),
        "pid": os.getpid(),
        "argv": sys.argv[1:],
        "init": STATE["init"],
    }


def get(params):
    id = params.get("id")
    if id == "boom":
        return None, {"code": FAILED, "message": BOOM_MESSAGE}
    if id == "missing":
        return {}, None
    if id == "echo":
        return {"task": task(fields=echo_fields())}, None
    return {"task": task(id)}, None


def apply(params):
    op = params.get("op")
    if op == "conflict":
        return {"outcome": "conflict", "current": task()}
    if op == "refused":
        return {"outcome": "refused", "message": REFUSED_MESSAGE}
    return {"outcome": "ok", "task": task(), "version": "v2", "files": ["a.md"], "items": [effect()]}


def preview(params):
    op = params.get("op")
    if op == "conflict":
        return {"outcome": "conflict", "current": task()}
    if op == "refused":
        return {"outcome": "refused", "message": REFUSED_MESSAGE}
    return {"outcome": "ok", "items": [effect()], "files": ["a.md"]}


def notifications():
    where = {"checkout": "main", "path": "a/ts-1.md"}
    return [
        (
            "task.changed",
            {"key": "fake:1", "version": "v1", "from": None, "to": state(), "diff": {}, "cause": "worktree"},
        ),
        ("task.renamed", {"from": "fake:1", "to": "fake:2"}),
        ("task.removed", {"key": "fake:2", "cause": "deleted"}),
        ("task.invalid", {"path": "a/bad.md", "checkout": None, "reason": "frontmatter over its cap"}),
        ("task.collision", {"shortId": "1", "locations": [where, {"checkout": "wt-1", "path": "b/ts-1.md"}]}),
        ("source.stale", {"reason": "rate limited"}),
    ]


def send_events():
    for method, params in notifications():
        say({"jsonrpc": "2.0", "method": method, "params": params})


def answer(method, params):
    """The `result` for a request, or `(None, error)`."""
    if method == "initialize":
        STATE["init"] = params
        return initialize_result(), None
    if method == "list":
        return {"tasks": [task()], "collections": [], "invalid": []}, None
    if method == "get":
        return get(params)
    if method == "apply":
        return apply(params), None
    if method == "preview":
        return preview(params), None
    if method == "watch":
        return {}, None
    return None, {"code": NOT_FOUND, "message": "Method not found: " + method}


def serve(line):
    request = json.loads(line)
    method, id, params = request["method"], request["id"], request.get("params", {})
    if "noise" in MODES:
        sys.stderr.write("fake provider: noise on %s\n" % method)
        sys.stderr.flush()
    if MODES.get("crash-on") == method:
        sys.exit(4)
    once_method, _, once_flag = MODES.get("crash-once", "").partition(":")
    if once_method == method and not os.path.exists(once_flag):
        open(once_flag, "w").close()
        sys.exit(4)
    if MODES.get("hang-on") == method:
        time.sleep(HANG_SECONDS)
        sys.exit(6)
    if MODES.get("events-before") == method:
        send_events()
    if MODES.get("garbage-on") == method:
        sys.stdout.write("this is not json\n")
        sys.stdout.flush()
    elif MODES.get("unknown") == method:
        say({"jsonrpc": "2.0", "id": id, "error": {"code": NOT_FOUND, "message": "Method not found: " + method}})
    else:
        result, error = answer(method, params)
        if error:
            say({"jsonrpc": "2.0", "id": id, "error": error})
        else:
            say({"jsonrpc": "2.0", "id": id, "result": result})
    if MODES.get("events-after") == method:
        send_events()
    if method != "initialize":
        STATE["replies"] += 1
        if "exit-after" in MODES and STATE["replies"] >= int(MODES["exit-after"]):
            sys.exit(3)


def main():
    while True:
        line = sys.stdin.readline()
        if not line:
            if "ignore-eof" in MODES:
                time.sleep(HANG_SECONDS)
            return
        if line.strip():
            serve(line)


main()
