#!/usr/bin/python3
"""Fake ACP child: replays a recorded JSON-RPC conversation as an ACP agent (ts-0086-tm).

    fake_acp.py <recording.jsonl> [more words]

It ignores the words after the recording, so it can stand in for an agent under any launch line. The recording is a
JSON Lines file. A blank line, and a line that starts with `#`, are skipped. Every other line is one step, run in order:

    {"say": {...}}          write the object to stdout as one line of JSON-RPC
    {"say_text": "..."}     write the text to stdout as one line that is not JSON
    {"stderr": "..."}       write the text to stderr as one line
    {"expect": {...}}       read one line of stdin and check it holds this JSON (see below)
    {"expect": {...}, "as": "name"}
                            the same; when the line is a request (it has a `method` and an `id`), remember its `id`
                            under the name
    {"wait_eof": true}      read stdin until it closes
    {"sleep": SECONDS}      do nothing for that long
    {"exit": N}             exit with code N now

`{"$id": "name"}` anywhere inside a later `say` or `expect` is replaced by the id remembered under the name, so a
recording can answer a request whatever id the host gave it (an ACP client numbers its own requests). The agent's own
ids, such as a permission request's, are written in the recording as they are.

`expect` is a subset match: every key in the expected object must be in the line, with a value that matches, and the
line may hold more. A list matches a list of the same length, element by element. Exit codes:

    96  a step the child does not know, or a recording line that is not JSON
    97  a line that did not match, or is not JSON: the wanted object and the line are on stderr
    98  stdin closed while an `expect` waited
    99  an `$id` that was never remembered

After the last step the child waits for stdin to close and exits 0, as an ACP agent does when its client goes away
while it is idle (S7, measured).
"""

import json
import sys
import time

EXIT_STEP_UNKNOWN = 96
EXIT_MISMATCH = 97
EXIT_STDIN_CLOSED = 98
EXIT_ID_UNKNOWN = 99


def stop(code, message):
    sys.stdout.flush()
    sys.stderr.write("fake acp: %s\n" % message)
    sys.stderr.flush()
    sys.exit(code)


def matches(want, got):
    if isinstance(want, dict):
        return isinstance(got, dict) and all(k in got and matches(v, got[k]) for k, v in want.items())
    if isinstance(want, list):
        return isinstance(got, list) and len(want) == len(got) and all(matches(w, g) for w, g in zip(want, got))
    return want == got


def steps_of(path):
    with open(path) as recording:
        for number, line in enumerate(recording, 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            try:
                yield number, json.loads(line)
            except ValueError:
                stop(EXIT_STEP_UNKNOWN, "step %d is not JSON: %s" % (number, line))


def write_line(stream, text):
    stream.write(text + "\n")
    stream.flush()


def compact(value):
    return json.dumps(value, separators=(",", ":"))


def resolve(number, value, ids):
    """`value` with every `{"$id": name}` replaced by the id remembered under the name."""
    if isinstance(value, dict):
        if list(value.keys()) == ["$id"]:
            if value["$id"] not in ids:
                stop(EXIT_ID_UNKNOWN, "step %d uses the id %s, which no `as` remembered" % (number, value["$id"]))
            return ids[value["$id"]]
        return {k: resolve(number, v, ids) for k, v in value.items()}
    if isinstance(value, list):
        return [resolve(number, v, ids) for v in value]
    return value


def expect(number, step, ids):
    want = resolve(number, step["expect"], ids)
    line = sys.stdin.readline()
    if not line:
        stop(EXIT_STDIN_CLOSED, "stdin closed at step %d, which expects %s" % (number, compact(want)))
    try:
        got = json.loads(line)
    except ValueError:
        stop(
            EXIT_MISMATCH,
            "step %d expects %s, got a line that is not JSON: %s" % (number, compact(want), line.strip()),
        )
    if not matches(want, got):
        stop(EXIT_MISMATCH, "step %d expects %s, got %s" % (number, compact(want), compact(got)))
    if "as" in step and isinstance(got, dict) and "method" in got and "id" in got:
        ids[step["as"]] = got["id"]


def main():
    ids = {}
    for number, step in steps_of(sys.argv[1]):
        if not isinstance(step, dict):
            stop(EXIT_STEP_UNKNOWN, "step %d is not a step: %s" % (number, compact(step)))
        if "say" in step:
            write_line(sys.stdout, compact(resolve(number, step["say"], ids)))
        elif "say_text" in step:
            write_line(sys.stdout, step["say_text"])
        elif "stderr" in step:
            write_line(sys.stderr, step["stderr"])
        elif "expect" in step:
            expect(number, step, ids)
        elif "wait_eof" in step:
            sys.stdin.read()
        elif "sleep" in step:
            time.sleep(step["sleep"])
        elif "exit" in step:
            sys.stdout.flush()
            sys.exit(step["exit"])
        else:
            stop(EXIT_STEP_UNKNOWN, "step %d is not a step: %s" % (number, compact(step)))
    sys.stdin.read()


main()
