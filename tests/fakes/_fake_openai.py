#!/usr/bin/env python3
"""Server OpenAI mínimo para los fakes de la suite (sin red externa)."""
import json, os, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port = 0
model = os.environ.get("FAKE_MODEL_ID", "fake-model")
args = sys.argv[1:]
for i, a in enumerate(args):
    if a == "--port" and i + 1 < len(args):
        port = int(args[i + 1])
    if a == "--served-model-name" and i + 1 < len(args):
        model = args[i + 1]

class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass
    def do_GET(self):
        if self.path in ("/health", "/health/"):
            body = b'{"status":"ok"}'
        elif self.path.startswith("/v1/models"):
            body = json.dumps({"object": "list", "data": [{"id": model, "object": "model"}]}).encode()
        else:
            self.send_response(404); self.end_headers(); return
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers(); self.wfile.write(body)
    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(n)
        body = json.dumps({"id": "chatcmpl-fake", "object": "chat.completion",
                           "model": model,
                           "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"},
                                        "finish_reason": "stop"}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers(); self.wfile.write(body)

if port == 0:
    print("fake-openai: falta --port", file=sys.stderr); sys.exit(2)
HTTPServer(("127.0.0.1", port), H).serve_forever()
