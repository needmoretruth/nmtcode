#!/usr/bin/env bash
# Serves web/ on http://127.0.0.1:PORT (default 8080) for local use and testing. Browsers
# allow the camera on localhost without HTTPS. Build web/pkg first with scripts/build-web.sh.
#
#   scripts/serve-web.sh [PORT]

set -euo pipefail

port=${1:-8080}
if [[ ! "$port" =~ ^[0-9]+$ ]] || ((port < 1 || port > 65535)); then
  echo "usage: scripts/serve-web.sh [PORT]   (PORT from 1 to 65535, default 8080)" >&2
  exit 2
fi

cd "$(dirname "$0")/.."
if [[ ! -f web/pkg/nmtcode_wasm_bg.wasm ]]; then
  echo "web/pkg is missing: run scripts/build-web.sh first" >&2
  exit 1
fi

echo "Serving web/ on http://127.0.0.1:$port/ (Ctrl-C stops)"
# The handler states the MIME types that the page needs, whatever the system's MIME table says,
# and asks the browser not to cache, so a rebuilt web/pkg is loaded on the next reload.
exec python3 - "$port" <<'PY'
import functools
import http.server
import sys


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".html": "text/html; charset=utf-8",
        ".css": "text/css; charset=utf-8",
        ".js": "text/javascript; charset=utf-8",
        ".wasm": "application/wasm",
        ".svg": "image/svg+xml",
    }

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        super().end_headers()


port = int(sys.argv[1])
handler = functools.partial(Handler, directory="web")
with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as server:
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
PY
