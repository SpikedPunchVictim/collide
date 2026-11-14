"""Minimal OTLP/HTTP-JSON receiver for the spike (stdlib only). Every POST body is stored with the arrival time.

Receiver.rows holds {recv_ns, path, body}; the runner writes them to a JSONL file. Accepts gzip. Binds 127.0.0.1 only.
"""
import gzip
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Receiver:
    def __init__(self, port=0):
        self.rows = []
        self.lock = threading.Lock()
        rows, lock = self.rows, self.lock

        class H(BaseHTTPRequestHandler):
            def do_POST(self):
                n = int(self.headers.get("content-length") or 0)
                raw = self.rfile.read(n)
                recv = time.time_ns()
                if self.headers.get("content-encoding") == "gzip":
                    raw = gzip.decompress(raw)
                try:
                    body = json.loads(raw)
                except Exception:
                    body = {"raw_bytes": len(raw)}
                with lock:
                    rows.append({"recv_ns": recv, "path": self.path, "body": body})
                self.send_response(200)
                self.send_header("content-type", "application/json")
                self.end_headers()
                self.wfile.write(b"{}")

            def log_message(self, *a):
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", port), H)
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def start(self):
        self.thread.start()
        return self

    def stop(self):
        self.server.shutdown()
        self.server.server_close()

    def snapshot(self):
        with self.lock:
            return list(self.rows)


def attrs(kvs):
    """OTLP attribute list to a flat dict."""
    out = {}
    for kv in kvs or []:
        v = kv.get("value", {})
        out[kv["key"]] = next(iter(v.values())) if v else None
    return out


def log_records(rows):
    """Yield (recv_ns, event_time_ns, attrs) for every log record in the received bodies."""
    for r in rows:
        if r["path"] != "/v1/logs":
            continue
        for rl in r["body"].get("resourceLogs", []):
            for sl in rl.get("scopeLogs", []):
                for lr in sl.get("logRecords", []):
                    a = attrs(lr.get("attributes"))
                    a["_body"] = next(iter(lr.get("body", {}).values()), None) if lr.get("body") else None
                    yield r["recv_ns"], int(lr.get("timeUnixNano") or 0), a


def spans(rows):
    """Yield (recv_ns, span dict with attrs) for every span."""
    for r in rows:
        if r["path"] != "/v1/traces":
            continue
        for rs in r["body"].get("resourceSpans", []):
            for ss in rs.get("scopeSpans", []):
                for sp in ss.get("spans", []):
                    d = dict(sp)
                    d["attrs"] = attrs(sp.get("attributes"))
                    yield r["recv_ns"], d
