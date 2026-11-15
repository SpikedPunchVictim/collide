#!/usr/bin/python3
"""Fake Claude: replays a recorded stream-json script (ts-0062-rj).

    fake_claude.py [--emulate-hooks] <recording.jsonl> [the words of claude's own line]

It ignores the words of claude's own line, so it can stand in for `claude` under any launch line. With `--emulate-hooks`
it does what `claude` does at session start: it reads the file `--settings` names in those words and, for every
`SessionStart` command hook in it, writes the `hook_started` and `hook_response` lines that `claude` writes, with the
`stdout`, `stderr` and exit code of the command run by `/bin/sh -c`. A line with no `--settings` word, or whose file is
unreadable, writes none: that is `claude` ignoring the settings (ts-0068-wx). The recording is a
JSON Lines file. A blank line, and a line that starts with `#`, are skipped. Every other line is one step, run in order:

    {"say": {...}}          write the object to stdout as one line of stream-json
    {"say_text": "..."}     write the text to stdout as one line that is not JSON
    {"stderr": "..."}       write the text to stderr as one line
    {"expect": {...}}       read one line of stdin and check it holds this JSON (see below)
    {"wait_eof": true}      read stdin until it closes
    {"dump_env": "path"}    write this process's environment to the file, as one JSON object
    {"grandchild_dump_env": "path"}
                            start a child that writes its own environment to the file, and wait for it
    {"sleep": SECONDS}      do nothing for that long (a child that holds on until it is signalled)
    {"touch": "path"}       create the file (a mark that the steps before it ran)
    {"exit": N}             exit with code N now

`expect` is a subset match: every key in the expected object must be in the line, with a value that matches, and the
line may hold more. A list matches a list of the same length, element by element. A line that does not match, or is
not JSON, ends the process with exit code 97 and a message on stderr; stdin closing first ends it with 98. A step
that is none of the above ends it with 96. After the last step the process waits for stdin to close and exits 0, as
`claude` does once its last turn is done.
"""

import json
import os
import subprocess
import sys
import time

EXIT_STEP_UNKNOWN = 96
EXIT_MISMATCH = 97
EXIT_STDIN_CLOSED = 98


def stop(code, message):
    sys.stdout.flush()
    sys.stderr.write("fake claude: %s\n" % message)
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
            if line and not line.startswith("#"):
                yield number, json.loads(line)


def write_line(stream, text):
    stream.write(text + "\n")
    stream.flush()


def expect(number, want):
    line = sys.stdin.readline()
    if not line:
        stop(EXIT_STDIN_CLOSED, "stdin closed at step %d, which expects %s" % (number, json.dumps(want)))
    try:
        got = json.loads(line)
    except ValueError:
        stop(EXIT_MISMATCH, "step %d expects %s, got a line that is not JSON: %s" % (number, json.dumps(want), line.strip()))
    if not matches(want, got):
        stop(EXIT_MISMATCH, "step %d expects %s, got %s" % (number, json.dumps(want), json.dumps(got)))


def session_start_commands(words):
    """The commands of the `SessionStart` command hooks in the file `--settings` names in `words`."""
    if "--settings" not in words or words.index("--settings") + 1 >= len(words):
        return []
    try:
        with open(words[words.index("--settings") + 1]) as settings:
            hooks = json.load(settings).get("hooks", {}).get("SessionStart", [])
    except (OSError, ValueError):
        return []
    return [h["command"] for group in hooks for h in group.get("hooks", []) if h.get("type") == "command"]


def emulate_session_start_hooks(words):
    for number, command in enumerate(session_start_commands(words), 1):
        base = {"hook_id": "hook-%d" % number, "hook_name": "SessionStart:startup", "hook_event": "SessionStart"}
        write_line(sys.stdout, json.dumps(dict(base, type="system", subtype="hook_started"), separators=(",", ":")))
        ran = subprocess.run(["/bin/sh", "-c", command], capture_output=True, text=True)
        response = dict(
            base,
            type="system",
            subtype="hook_response",
            output=ran.stdout + ran.stderr,
            stdout=ran.stdout,
            stderr=ran.stderr,
            exit_code=ran.returncode,
            outcome="success" if ran.returncode == 0 else "error",
        )
        write_line(sys.stdout, json.dumps(response, separators=(",", ":")))


def main():
    words = sys.argv[1:]
    emulate = bool(words) and words[0] == "--emulate-hooks"
    if emulate:
        words = words[1:]
        emulate_session_start_hooks(words[1:])
    for number, step in steps_of(words[0]):
        if "say" in step:
            write_line(sys.stdout, json.dumps(step["say"], separators=(",", ":")))
        elif "say_text" in step:
            write_line(sys.stdout, step["say_text"])
        elif "stderr" in step:
            write_line(sys.stderr, step["stderr"])
        elif "expect" in step:
            expect(number, step["expect"])
        elif "wait_eof" in step:
            sys.stdin.read()
        elif "dump_env" in step:
            with open(step["dump_env"], "w") as out:
                json.dump(dict(os.environ), out)
        elif "grandchild_dump_env" in step:
            subprocess.run(
                [sys.executable, "-c", "import json, os, sys; json.dump(dict(os.environ), open(sys.argv[1], 'w'))", step["grandchild_dump_env"]],
                check=True,
            )
        elif "sleep" in step:
            time.sleep(step["sleep"])
        elif "touch" in step:
            open(step["touch"], "w").close()
        elif "exit" in step:
            sys.stdout.flush()
            sys.exit(step["exit"])
        else:
            stop(EXIT_STEP_UNKNOWN, "step %d is not a step: %s" % (number, json.dumps(step)))
    sys.stdin.read()


main()
