#!/usr/bin/python3
"""Fake Claude: replays a recorded stream-json script (ts-0062-rj).

    fake_claude.py <recording.jsonl>

It ignores the rest of its command line, so it can stand in for `claude` under any launch line. The recording is a
JSON Lines file. A blank line, and a line that starts with `#`, are skipped. Every other line is one step, run in order:

    {"say": {...}}          write the object to stdout as one line of stream-json
    {"say_text": "..."}     write the text to stdout as one line that is not JSON
    {"stderr": "..."}       write the text to stderr as one line
    {"expect": {...}}       read one line of stdin and check it holds this JSON (see below)
    {"wait_eof": true}      read stdin until it closes
    {"dump_env": "path"}    write this process's environment to the file, as one JSON object
    {"grandchild_dump_env": "path"}
                            start a child that writes its own environment to the file, and wait for it
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


def main():
    for number, step in steps_of(sys.argv[1]):
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
        elif "exit" in step:
            sys.stdout.flush()
            sys.exit(step["exit"])
        else:
            stop(EXIT_STEP_UNKNOWN, "step %d is not a step: %s" % (number, json.dumps(step)))
    sys.stdin.read()


main()
