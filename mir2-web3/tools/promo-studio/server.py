"""Loopback-only HTTP frontend for the local FFmpeg promo studio."""
from __future__ import annotations

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import mimetypes
from pathlib import Path
import re
import socket
import threading
import urllib.parse
import uuid

from engine import BASE, IMAGE_EXTENSIONS, MAX_ASSET_BYTES, PROVENANCE_KINDS, VIDEO_EXTENSIONS, Studio, StudioError


def parse_range(value: str | None, size: int) -> tuple[int, int] | None:
    if value is None:
        return None
    match = re.fullmatch(r"bytes=(\d*)-(\d*)", value.strip())
    if not match or not (match[1] or match[2]) or size <= 0:
        raise StudioError("Invalid or unsatisfiable media range", 416)
    if not match[1]:
        suffix = int(match[2])
        if suffix <= 0:
            raise StudioError("Invalid or unsatisfiable media range", 416)
        return max(0, size - suffix), size - 1
    start = int(match[1])
    end = int(match[2]) if match[2] else size - 1
    if start >= size or end < start:
        raise StudioError("Invalid or unsatisfiable media range", 416)
    return start, min(end, size - 1)


class StudioHTTPServer(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 16

    def __init__(self, address, studio, web_dir=None):
        if address[0] not in ("127.0.0.1", "localhost"):
            raise ValueError("Promo Studio may only listen on loopback")
        self.studio = studio
        self.web_dir = Path(web_dir or BASE / "web").resolve()
        self.upload_slots = threading.BoundedSemaphore(2)
        super().__init__(("127.0.0.1", address[1]), Handler)


class Handler(BaseHTTPRequestHandler):
    server_version = "LocalPromoStudio/1.0"
    protocol_version = "HTTP/1.1"

    def setup(self):
        super().setup()
        self.connection.settimeout(20)

    def log_message(self, format, *args):
        # Request bodies, local import paths and provider secrets are not logged.
        pass

    def _guard(self, mutating=False):
        port = self.server.server_port
        allowed_hosts = {f"127.0.0.1:{port}", f"localhost:{port}"}
        if port == 80:
            allowed_hosts |= {"127.0.0.1", "localhost"}
        if self.headers.get("Host", "").lower() not in allowed_hosts or len(self.headers.get_all("Host", [])) != 1:
            raise StudioError("Use this studio's loopback host and port", 403)
        if mutating:
            allowed_origins = {"http://" + host for host in allowed_hosts}
            if self.headers.get("Origin", "") not in allowed_origins or len(self.headers.get_all("Origin", [])) != 1:
                raise StudioError("Mutating requests require the local studio Origin", 403)
            if self.headers.get("Sec-Fetch-Site", "") == "cross-site":
                raise StudioError("Cross-site requests are not permitted", 403)
        url = urllib.parse.urlsplit(self.path)
        if url.scheme or url.netloc or "\x00" in self.path:
            raise StudioError("Use a relative local request URL", 400)
        return urllib.parse.unquote(url.path)

    def _headers(self, status, content_type, length, cache="no-store", extra=None):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(length))
        self.send_header("Cache-Control", cache)
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("Content-Security-Policy", "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; media-src 'self'; base-uri 'none'; frame-ancestors 'none'")
        for key, value in (extra or {}).items():
            self.send_header(key, value)
        self.end_headers()

    def _json(self, payload, status=200, head=False):
        encoded = json.dumps(payload, ensure_ascii=False, allow_nan=False).encode("utf-8")
        self._headers(status, "application/json; charset=utf-8", len(encoded))
        if not head:
            self.wfile.write(encoded)

    def _body(self):
        if self.headers.get("Transfer-Encoding"):
            raise StudioError("Transfer-Encoding is not supported", 400)
        if self.headers.get("Content-Type", "").split(";")[0].strip().lower() != "application/json":
            raise StudioError("Use Content-Type application/json", 415)
        if len(self.headers.get_all("Content-Length", [])) != 1:
            raise StudioError("A single Content-Length is required", 411)
        try:
            length = int(self.headers["Content-Length"])
        except ValueError as exc:
            raise StudioError("Invalid Content-Length") from exc
        if length < 2 or length > 65536:
            raise StudioError("JSON request size must be 2–65536 bytes", 413)
        try:
            data = self.rfile.read(length)
            if len(data) != length:
                raise StudioError("Incomplete request body")
            def invalid_constant(value):
                raise ValueError(value)
            return json.loads(data, parse_constant=invalid_constant)
        except (UnicodeError, ValueError) as exc:
            raise StudioError("The request body must contain valid finite JSON") from exc

    def do_POST(self):
        try:
            path = self._guard(mutating=True)
            if path == "/api/upload":
                self._json(self._upload(), 201)
                return
            payload = self._body()
            if path == "/api/assets":
                self._json(self.server.studio.import_asset(payload), 201)
            elif path == "/api/jobs":
                self._json(self.server.studio.enqueue(payload), 202)
            elif path == "/api/brief":
                self._json(self.server.studio.brief(payload))
            else:
                raise StudioError("Unknown API route", 404)
        except StudioError as exc:
            self.close_connection = True
            self._json({"error": str(exc)}, exc.status)
        except (socket.timeout, ConnectionError, BrokenPipeError):
            self.close_connection = True
        except Exception:
            self.close_connection = True
            self._json({"error": "The local studio could not complete this request"}, 500)

    def _upload(self):
        if self.headers.get("Transfer-Encoding"):
            raise StudioError("Transfer-Encoding is not supported", 400)
        if self.headers.get("Content-Type", "").split(";")[0].strip().lower() != "application/octet-stream":
            raise StudioError("Binary uploads require Content-Type application/octet-stream", 415)
        if len(self.headers.get_all("Content-Length", [])) != 1:
            raise StudioError("A single Content-Length is required", 411)
        try:
            length = int(self.headers["Content-Length"])
        except ValueError as exc:
            raise StudioError("Invalid Content-Length") from exc
        if not 0 < length <= MAX_ASSET_BYTES:
            raise StudioError("Upload a file between 1 byte and 2 GB", 413)
        query = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query, keep_blank_values=True, max_num_fields=10)
        if any(len(values) != 1 for values in query.values()) or any(key not in ("filename", "label", "kind", "source", "tags") for key in query):
            raise StudioError("Use single filename, label, kind, source and tags upload parameters")
        params = {key: values[0] for key, values in query.items()}
        filename = params.get("filename", "")
        if not filename or len(filename) > 240 or Path(filename).name != filename or any(char in filename for char in ("/", "\\", ":", "\x00")):
            raise StudioError("A simple local media filename is required")
        extension = Path(filename).suffix.lower()
        if extension not in IMAGE_EXTENSIONS | VIDEO_EXTENSIONS:
            raise StudioError("Unsupported upload file extension")
        kind = params.get("kind")
        if kind not in PROVENANCE_KINDS:
            raise StudioError("Supply kind ai_creative, gameplay or game_screenshot")
        if not self.server.upload_slots.acquire(blocking=False):
            raise StudioError("Two uploads are already active; wait for one to finish", 429)
        temporary = None
        try:
            directory = self.server.studio.data_dir / "uploads"
            directory.mkdir(exist_ok=True)
            if directory.is_symlink() or not directory.resolve().is_relative_to(self.server.studio.data_dir):
                raise StudioError("The managed upload directory is not safe", 500)
            temporary = directory / (uuid.uuid4().hex + extension)
            with temporary.open("xb") as handle:
                remaining = length
                while remaining:
                    block = self.rfile.read(min(1024 * 1024, remaining))
                    if not block:
                        raise StudioError("The upload ended before the file was complete")
                    handle.write(block)
                    remaining -= len(block)
            payload = {"path": str(temporary), "label": params.get("label") or Path(filename).stem, "provenance": {"kind": kind, "source": params.get("source") or "Browser-uploaded local file: " + filename}, "tags": [tag for tag in params.get("tags", "").split(",") if tag]}
            asset = self.server.studio.import_asset(payload, original_filename=filename)
            return asset
        finally:
            # Only this request's generated temporary path is ever removed.
            if temporary is not None:
                temporary.unlink(missing_ok=True)
            self.server.upload_slots.release()

    def do_HEAD(self):
        self._get(head=True)

    def do_GET(self):
        self._get()

    def _get(self, head=False):
        try:
            path = self._guard()
            if path == "/api/status":
                self._json(self.server.studio.status(), head=head)
                return
            if path.startswith("/api/"):
                raise StudioError("Unknown API route", 404)
            if path.startswith("/media/"):
                pieces = path.split("/")
                target = self.server.studio.media_path(pieces[2], pieces[3]) if len(pieces) == 4 else None
                if not target:
                    raise StudioError("Media file not found", 404)
                self._file(target, head, allow_range=True)
                return
            name = "index.html" if path == "/" else path.removeprefix("/")
            if "\\" in name or "\x00" in name or ":" in name or any(p in (".", "..") for p in Path(name).parts):
                raise StudioError("Page not found", 404)
            target = self.server.web_dir / name
            if not target.is_file() or target.is_symlink() or not target.resolve().is_relative_to(self.server.web_dir):
                raise StudioError("Page not found", 404)
            self._file(target, head)
        except StudioError as exc:
            self._json({"error": str(exc)}, exc.status, head=head)
        except (socket.timeout, ConnectionError, BrokenPipeError):
            self.close_connection = True
        except Exception:
            self._json({"error": "The local studio could not read this resource"}, 500, head=head)

    def _file(self, target, head=False, allow_range=False):
        size = target.stat().st_size
        try:
            byte_range = parse_range(self.headers.get("Range"), size) if allow_range else None
        except StudioError:
            self._headers(416, "text/plain; charset=utf-8", 0, extra={"Content-Range": f"bytes */{size}", "Accept-Ranges": "bytes"})
            return
        start, end = byte_range if byte_range else (0, size - 1)
        length = end - start + 1
        content_type = {".srt": "application/x-subrip; charset=utf-8", ".json": "application/json; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8", ".html": "text/html; charset=utf-8"}.get(target.suffix.lower(), mimetypes.guess_type(target.name)[0] or "application/octet-stream")
        extra = {"Accept-Ranges": "bytes"} if allow_range else {}
        if byte_range:
            extra["Content-Range"] = f"bytes {start}-{end}/{size}"
        self._headers(206 if byte_range else 200, content_type, length, extra=extra)
        if head:
            return
        with target.open("rb") as handle:
            handle.seek(start)
            while length > 0:
                block = handle.read(min(length, 256 * 1024))
                if not block:
                    break
                self.wfile.write(block)
                length -= len(block)


def main():
    parser = argparse.ArgumentParser(description="Local FFmpeg promo video studio (no publishing or provider spend by default)")
    parser.add_argument("--host", choices=("127.0.0.1", "localhost"), default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3921)
    parser.add_argument("--data-dir", help="Own local output directory; otherwise PROMO_STUDIO_DATA or artifacts/promo-studio")
    args = parser.parse_args()
    studio = Studio(data_dir=args.data_dir)
    server = StudioHTTPServer((args.host, args.port), studio)
    print(f"Promo Studio: http://127.0.0.1:{server.server_port}\nData: {studio.data_dir}\nProvider: {studio.provider_status()['mode']}", flush=True)
    try:
        server.serve_forever(poll_interval=.5)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        studio.close()


if __name__ == "__main__":
    main()
