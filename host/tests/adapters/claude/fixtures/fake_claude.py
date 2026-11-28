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
    {"dump_launch": "path"} write what `claude` was started with to the file, as one JSON object: `argv` (the words of
                            claude's own line, after the recording), `env` (the environment) and `cwd` (the working
                            folder), as the repository-instructions measurement reads them (ts-0103-sa)
    {"grandchild_dump_env": "path"}
                            start a child that writes its own environment to the file, and wait for it
    {"sleep": SECONDS}      do nothing for that long (a child that holds on until it is signalled)
    {"touch": "path"}       create the file (a mark that the steps before it ran)
    {"wait_file": "path"}   wait until the file exists, which the test makes when the phase may go on (ts-0188-kb). It
                            ends with 96 after two minutes, so a test that never makes it does not leave a child
    {"write_file": {"path": "a/b.txt", "text": "..."}}
                            write the text to the file, below this process's working folder (ts-0078-ye)
    {"git_commit": "message"}
                            stage everything in the working folder and commit it, as an agent does (ts-0078-ye)
    {"git": ["merge", "--no-commit", "main"]}
                            run git in the working folder, as `git_commit` does, and go on whatever it answers: a merge
                            that stops on a conflict is the point of it (ts-0080-tq)
    {"init": true}          write the `system` `init` line `claude` writes first (ts-0201-us)
    {"assistant": "text"}   write an assistant message with this text, with a little usage (ts-0201-us)
    {"result": {"subtype": "success", "is_error": false, "api_error_status": null, "cost": 0.0041, "text": "..."}}
                            write the `result` line that ends a turn; every key is optional (ts-0201-us)
    {"write_pid": "name"}   write this process's pid to the file `name`, below the working folder (ts-0201-us)
    {"write_stdin": "name"} read stdin until it closes and write what came to the file `name`, below the working
                            folder: the request an ask sends once as text (ts-0201-us)
    {"dump_launch": "name"} write the words this process got and its environment, as JSON, to the file `name`, below
                            the working folder (ts-0201-us)
    {"report_env": "NAME"}  say, as an assistant message, whether the variable is set, with its length and a sum of its
                            bytes, and never its value (ts-0201-us)
    {"tools_call": {"name": "query", "arguments": {...}}}
                            call a tool of the endpoint the `--mcp-config` file names: post `tools/call` to its `url`
                            with its headers, wait for the answer, and write the call and its result into the stream,
                            as `claude` writes a `tool_use` and its `tool_result` (ts-0201-us)
    {"exit": N}             exit with code N now

In the text of a `say`, `say_text` or `assistant`, `{{mcp_token}}` is the bearer token of the `--mcp-config` file's
server and `{{mcp_url}}` is its URL: an agent that echoes what it was given (ts-0201-us).

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
import urllib.request

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


def write_file(path, text):
    """Writes below the working folder; a path that leaves it is a mistake in the recording."""
    if os.path.isabs(path) or ".." in path.split("/"):
        stop(EXIT_STEP_UNKNOWN, "write_file takes a path below the working folder, not %s" % path)
    os.makedirs(os.path.dirname(os.path.join(os.getcwd(), path)), exist_ok=True)
    with open(path, "w") as out:
        out.write(text)


WAIT_FILE_SECONDS = 120


def wait_file(path):
    """Holds the phase until the test makes the file; a recording whose file never comes ends instead of hanging."""
    deadline = time.monotonic() + WAIT_FILE_SECONDS
    while not os.path.exists(path):
        if time.monotonic() > deadline:
            stop(EXIT_STEP_UNKNOWN, "wait_file: %s did not appear in %d s" % (path, WAIT_FILE_SECONDS))
        time.sleep(0.05)


IDENTITY = ["-c", "user.name=Fake Claude", "-c", "user.email=fake@example.invalid", "-c", "commit.gpgsign=false"]


def git_commit(message):
    """Stages and commits everything, with an identity of its own and none of the user's or the system's git config."""
    for args in (["add", "-A"], IDENTITY + ["commit", "-q", "-m", message]):
        ran = subprocess.run(["git"] + args, env=git_env(), capture_output=True, text=True)
        if ran.returncode != 0:
            stop(EXIT_STEP_UNKNOWN, "git %s failed: %s" % (" ".join(args), ran.stderr.strip()))


def git_env():
    env = dict(os.environ, HOME="/nonexistent", GIT_CONFIG_GLOBAL="/dev/null")
    env.update(GIT_CONFIG_SYSTEM="/dev/null", GIT_CONFIG_NOSYSTEM="1")
    return env


def git_run(args):
    """Runs git with the arguments, with the identity of `git_commit`. Its exit code is not looked at."""
    subprocess.run(["git"] + IDENTITY + args, env=git_env(), capture_output=True, text=True)


def below_cwd(path):
    if os.path.isabs(path) or ".." in path.split("/"):
        stop(EXIT_STEP_UNKNOWN, "a file below the working folder is wanted, not %s" % path)
    return path


def mcp_server(words):
    """The server of the `--mcp-config` file in `words`: its `url` and its `headers`."""
    if "--mcp-config" not in words or words.index("--mcp-config") + 1 >= len(words):
        stop(EXIT_STEP_UNKNOWN, "there is no --mcp-config in claude's line")
    with open(words[words.index("--mcp-config") + 1]) as config:
        servers = json.load(config)["mcpServers"]
    return next(iter(servers.values()))


def substitute(text, words):
    """`{{mcp_token}}` and `{{mcp_url}}` in `text`, from the `--mcp-config` file."""
    if "{{mcp_" not in text:
        return text
    server = mcp_server(words)
    token = server.get("headers", {}).get("Authorization", "").replace("Bearer ", "", 1)
    return text.replace("{{mcp_token}}", token).replace("{{mcp_url}}", server["url"])


def assistant_text(text):
    return {
        "type": "assistant",
        "message": {
            "model": "claude-haiku-5-5", "id": "msg_fake", "type": "message", "role": "assistant",
            "content": [{"type": "text", "text": text}], "stop_reason": None,
            "usage": USAGE,
        },
        "parent_tool_use_id": None,
        "session_id": SESSION,
    }


SESSION = "7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17"
USAGE = {"input_tokens": 4, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 12000, "output_tokens": 40,
         "service_tier": "standard"}


def say_json(value):
    write_line(sys.stdout, json.dumps(value, separators=(",", ":")))


def say_init():
    say_json({"type": "system", "subtype": "init", "cwd": os.getcwd(), "session_id": SESSION,
              "tools": ["Read", "Grep", "Glob"], "mcp_servers": [], "model": "claude-haiku-5-5",
              "permissionMode": "default", "apiKeySource": "none", "claude_code_version": "2.1.323"})


def say_result(spec):
    failed = bool(spec.get("is_error", False))
    errors = [spec["errors"]] if "errors" in spec else []
    say_json({"type": "result", "subtype": spec.get("subtype", "success"), "is_error": failed,
              "api_error_status": spec.get("api_error_status"), "duration_ms": 2100, "duration_api_ms": 1300,
              "num_turns": 1, "result": spec.get("text"), "stop_reason": None if failed else "end_turn",
              "session_id": SESSION, "total_cost_usd": spec.get("cost", 0.0041), "usage": USAGE,
              "permission_denials": [], "terminal_reason": "completed", "errors": errors})


def report_env(name):
    value = os.environ.get(name)
    if value is None:
        text = "env %s: absent" % name
    else:
        text = "env %s: set, length %d, sum %d" % (name, len(value), sum(value.encode()) % 65521)
    say_json(assistant_text(text))


def tools_call(words, call, number):
    """Posts the call to the endpoint and writes the call and what came back as a `tool_use` and a `tool_result`."""
    server = mcp_server(words)
    use_id = "toolu_mcp_%d" % number
    name = "mcp__collide__" + call["name"]
    session = SESSION
    use = {
        "type": "assistant",
        "message": {"model": "claude-haiku-5-5", "id": "msg_call_%d" % number, "type": "message", "role": "assistant",
                    "content": [{"type": "tool_use", "id": use_id, "name": name, "input": call.get("arguments", {})}],
                    "stop_reason": None,
                    "usage": USAGE},
        "parent_tool_use_id": None, "session_id": session,
    }
    write_line(sys.stdout, json.dumps(use, separators=(",", ":")))
    body = json.dumps({"jsonrpc": "2.0", "id": number, "method": "tools/call",
                       "params": {"name": call["name"], "arguments": call.get("arguments", {})}}).encode()
    headers = dict(server.get("headers", {}))
    headers["Content-Type"] = "application/json"
    request = urllib.request.Request(server["url"], data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(request, timeout=60) as answer:
            reply, status = json.load(answer), answer.status
    except urllib.error.HTTPError as error:
        reply, status = {"error": {"message": "HTTP %d" % error.code}}, error.code
    failed = {"content": [{"type": "text", "text": json.dumps(reply.get("error"))}], "isError": True}
    result = reply.get("result") or failed
    text = "".join(part.get("text", "") for part in result.get("content", []))
    done = {
        "type": "user",
        "message": {"role": "user", "content": [{"type": "tool_result", "content": text,
                                                 "is_error": bool(result.get("isError")), "tool_use_id": use_id}]},
        "parent_tool_use_id": None, "session_id": session,
    }
    write_line(sys.stdout, json.dumps(done, separators=(",", ":")))


def main():
    words = sys.argv[1:]
    emulate = bool(words) and words[0] == "--emulate-hooks"
    if emulate:
        words = words[1:]
        emulate_session_start_hooks(words[1:])
    for number, step in steps_of(words[0]):
        if "say" in step:
            write_line(sys.stdout, substitute(json.dumps(step["say"], separators=(",", ":")), words[1:]))
        elif "say_text" in step:
            write_line(sys.stdout, substitute(step["say_text"], words[1:]))
        elif "stderr" in step:
            write_line(sys.stderr, step["stderr"])
        elif "expect" in step:
            expect(number, step["expect"])
        elif "wait_eof" in step:
            sys.stdin.read()
        elif "dump_env" in step:
            with open(step["dump_env"], "w") as out:
                json.dump(dict(os.environ), out)
        elif "dump_launch" in step:
            with open(step["dump_launch"], "w") as out:
                json.dump({"argv": words[1:], "env": dict(os.environ), "cwd": os.getcwd()}, out)
        elif "grandchild_dump_env" in step:
            subprocess.run(
                [sys.executable, "-c", "import json, os, sys; json.dump(dict(os.environ), open(sys.argv[1], 'w'))", step["grandchild_dump_env"]],
                check=True,
            )
        elif "sleep" in step:
            time.sleep(step["sleep"])
        elif "touch" in step:
            open(step["touch"], "w").close()
        elif "wait_file" in step:
            wait_file(step["wait_file"])
        elif "write_file" in step:
            write_file(step["write_file"]["path"], step["write_file"]["text"])
        elif "git_commit" in step:
            git_commit(step["git_commit"])
        elif "git" in step:
            git_run(step["git"])
        elif "init" in step:
            say_init()
        elif "assistant" in step:
            say_json(assistant_text(substitute(step["assistant"], words[1:])))
        elif "result" in step:
            say_result(step["result"])
        elif "write_pid" in step:
            with open(below_cwd(step["write_pid"]), "w") as out:
                out.write(str(os.getpid()))
        elif "write_stdin" in step:
            with open(below_cwd(step["write_stdin"]), "w") as out:
                out.write(sys.stdin.read())
        elif "dump_launch" in step:
            with open(below_cwd(step["dump_launch"]), "w") as out:
                json.dump({"argv": words, "env": dict(os.environ)}, out)
        elif "report_env" in step:
            report_env(step["report_env"])
        elif "tools_call" in step:
            tools_call(words[1:], step["tools_call"], number)
        elif "exit" in step:
            sys.stdout.flush()
            sys.exit(step["exit"])
        else:
            stop(EXIT_STEP_UNKNOWN, "step %d is not a step: %s" % (number, json.dumps(step)))
    sys.stdin.read()


main()
