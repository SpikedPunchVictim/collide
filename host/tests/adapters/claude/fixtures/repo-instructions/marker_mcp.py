#!/usr/bin/python3
"""A stand-in MCP server for the repository-instructions measurement (ts-0103-sa).

    marker_mcp.py <marker file>

It creates the marker file when it starts, which shows that `claude` started it, and then answers the MCP handshake
over stdio (`initialize`, `tools/list`, anything else with an empty result) so `claude` does not wait on it. It offers
no tool and ends when stdin closes.
"""

import json
import sys

open(sys.argv[1], "w").close()

for line in sys.stdin:
    try:
        message = json.loads(line)
    except ValueError:
        continue
    if "id" not in message:
        continue
    method = message.get("method")
    if method == "initialize":
        result = {
            "protocolVersion": message.get("params", {}).get("protocolVersion", "2024-11-05"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "marker-server", "version": "0"},
        }
    elif method == "tools/list":
        result = {"tools": []}
    else:
        result = {}
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": result}) + "\n")
    sys.stdout.flush()
