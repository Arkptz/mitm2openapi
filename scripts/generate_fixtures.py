#!/tmp/mitmproxy-venv/bin/python3
"""Generate test fixtures for mitm2openapi Rust project.

Creates:
  - testdata/flows/*.flow  — mitmproxy flow dump files (tnetstring encoded)
  - testdata/har/*.har     — HAR 1.2 JSON files
  - testdata/expected/*.yaml — golden OpenAPI 3.0 YAML files

Idempotent: safe to re-run.
"""

import json
import os
import time
from pathlib import Path
from uuid import uuid4

from ruamel.yaml import YAML
from mitmproxy.io import tnetstring

_yaml = YAML()
_yaml.default_flow_style = False

ROOT = Path(__file__).resolve().parent.parent
FLOWS_DIR = ROOT / "testdata" / "flows"
HAR_DIR = ROOT / "testdata" / "har"
EXPECTED_DIR = ROOT / "testdata" / "expected"


def ensure_dirs():
    for d in (FLOWS_DIR, HAR_DIR, EXPECTED_DIR):
        d.mkdir(parents=True, exist_ok=True)


# ---------------------------------------------------------------------------
# Flow helpers
# ---------------------------------------------------------------------------


def _base_flow(**overrides):
    """Return a mitmproxy v21 flow dict with sensible defaults."""
    now = time.time()
    flow = {
        "version": 21,
        "type": "http",
        "id": str(uuid4()),
        "error": None,
        "client_conn": {
            "id": str(uuid4()),
            "peername": ("127.0.0.1", 12345),
            "sockname": ("127.0.0.1", 8080),
            "timestamp_start": now - 1,
            "timestamp_tls_setup": now - 0.9,
            "timestamp_end": None,
            "state": 0,
            "tls_established": True,
            "tls_version": "TLSv1.3",
            "alpn": b"h2",
            "sni": "api.example.com",
            "tls_extensions": None,
            "certificate_list": [],
            "mitmcert": None,
            "proxy_mode": ("regular",),
        },
        "server_conn": {
            "id": str(uuid4()),
            "peername": ("93.184.216.34", 443),
            "sockname": ("127.0.0.1", 54321),
            "address": ("api.example.com", 443),
            "timestamp_start": now - 0.8,
            "timestamp_tcp_setup": now - 0.7,
            "timestamp_tls_setup": now - 0.6,
            "timestamp_end": None,
            "state": 0,
            "tls_established": True,
            "tls_version": "TLSv1.3",
            "alpn": b"h2",
            "sni": "api.example.com",
            "tls_extensions": None,
            "certificate_list": [],
            "via": None,
        },
        "intercepted": False,
        "is_replay": None,
        "marked": "",
        "metadata": {},
        "comment": "",
        "timestamp_created": now,
        "request": {
            "host": "api.example.com",
            "port": 443,
            "method": b"GET",
            "scheme": b"https",
            "authority": b"api.example.com",
            "path": b"/api/v1/users",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Host", b"api.example.com"],
                [b"Accept", b"application/json"],
                [b"User-Agent", b"TestClient/1.0"],
            ],
            "content": None,
            "trailers": None,
            "timestamp_start": now,
            "timestamp_end": now + 0.1,
        },
        "response": {
            "status_code": 200,
            "reason": b"OK",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Content-Type", b"application/json"],
            ],
            "content": b'{"users": [{"id": 1, "name": "Alice"}]}',
            "trailers": None,
            "timestamp_start": now + 0.05,
            "timestamp_end": now + 0.1,
        },
        "websocket": None,
        "backup": None,
    }
    # Apply overrides via deep-ish merge
    for k, v in overrides.items():
        if isinstance(v, dict) and isinstance(flow.get(k), dict):
            flow[k].update(v)
        else:
            flow[k] = v
    return flow


def _write_flows(path: Path, flows: list[dict]):
    """Write one or more flows to a .flow file (concatenated tnetstring)."""
    with open(path, "wb") as f:
        for flow in flows:
            f.write(tnetstring.dumps(flow))


# ---------------------------------------------------------------------------
# Flow fixtures
# ---------------------------------------------------------------------------


def gen_simple_get():
    flow = _base_flow()
    _write_flows(FLOWS_DIR / "simple_get.flow", [flow])


def gen_post_json():
    flow = _base_flow(
        request={
            "host": "api.example.com",
            "port": 443,
            "method": b"POST",
            "scheme": b"https",
            "authority": b"api.example.com",
            "path": b"/api/v1/users",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Host", b"api.example.com"],
                [b"Content-Type", b"application/json"],
                [b"Accept", b"application/json"],
            ],
            "content": b'{"name": "Bob", "email": "bob@example.com"}',
            "trailers": None,
            "timestamp_start": time.time(),
            "timestamp_end": time.time() + 0.1,
        },
        response={
            "status_code": 201,
            "reason": b"Created",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Content-Type", b"application/json"],
            ],
            "content": b'{"id": 2, "name": "Bob", "email": "bob@example.com"}',
            "trailers": None,
            "timestamp_start": time.time() + 0.05,
            "timestamp_end": time.time() + 0.1,
        },
    )
    _write_flows(FLOWS_DIR / "post_json.flow", [flow])


def gen_post_form():
    flow = _base_flow(
        request={
            "host": "api.example.com",
            "port": 443,
            "method": b"POST",
            "scheme": b"https",
            "authority": b"api.example.com",
            "path": b"/api/v1/login",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Host", b"api.example.com"],
                [b"Content-Type", b"application/x-www-form-urlencoded"],
            ],
            "content": b"username=alice&password=secret123",
            "trailers": None,
            "timestamp_start": time.time(),
            "timestamp_end": time.time() + 0.1,
        },
        response={
            "status_code": 200,
            "reason": b"OK",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Content-Type", b"application/json"],
            ],
            "content": b'{"token": "eyJhbGciOiJIUzI1NiJ9.test.sig"}',
            "trailers": None,
            "timestamp_start": time.time() + 0.05,
            "timestamp_end": time.time() + 0.1,
        },
    )
    _write_flows(FLOWS_DIR / "post_form.flow", [flow])


def gen_multi_status():
    now = time.time()
    flows = []

    specs = [
        (b"/api/v1/users", 200, b"OK", b'{"users": []}'),
        (b"/api/v1/users", 201, b"Created", b'{"id": 1}'),
        (b"/api/v1/users/999", 404, b"Not Found", b'{"error": "not found"}'),
        (
            b"/api/v1/internal",
            500,
            b"Internal Server Error",
            b'{"error": "server error"}',
        ),
    ]
    for path, status, reason, body in specs:
        flow = _base_flow()
        flow["request"]["path"] = path
        flow["response"]["status_code"] = status
        flow["response"]["reason"] = reason
        flow["response"]["content"] = body
        flows.append(flow)

    _write_flows(FLOWS_DIR / "multi_status.flow", flows)


def gen_no_response():
    flow = _base_flow()
    flow["response"] = None
    _write_flows(FLOWS_DIR / "no_response.flow", [flow])


def gen_non_utf8():
    flow = _base_flow(
        request={
            "host": "api.example.com",
            "port": 443,
            "method": b"GET",
            "scheme": b"https",
            "authority": b"api.example.com",
            "path": b"/api/v1/binary",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Host", b"api.example.com"],
                [b"Accept", b"application/octet-stream"],
            ],
            "content": None,
            "trailers": None,
            "timestamp_start": time.time(),
            "timestamp_end": time.time() + 0.1,
        },
        response={
            "status_code": 200,
            "reason": b"OK",
            "http_version": b"HTTP/1.1",
            "headers": [
                [b"Content-Type", b"application/octet-stream"],
            ],
            "content": b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\xff\xfe\xfd\xfc",
            "trailers": None,
            "timestamp_start": time.time() + 0.05,
            "timestamp_end": time.time() + 0.1,
        },
    )
    _write_flows(FLOWS_DIR / "non_utf8.flow", [flow])


def gen_multiple():
    """5+ flows in one file — the standard mitmproxy dump format."""
    now = time.time()
    paths_and_methods = [
        (b"GET", b"/api/v1/users"),
        (b"GET", b"/api/v1/users/1"),
        (b"POST", b"/api/v1/users"),
        (b"PUT", b"/api/v1/users/1"),
        (b"DELETE", b"/api/v1/users/1"),
        (b"GET", b"/api/v1/products"),
    ]

    flows = []
    for method, path in paths_and_methods:
        flow = _base_flow()
        flow["request"]["method"] = method
        flow["request"]["path"] = path

        if method == b"POST":
            flow["request"]["headers"].append([b"Content-Type", b"application/json"])
            flow["request"]["content"] = b'{"name": "New User"}'
            flow["response"]["status_code"] = 201
            flow["response"]["reason"] = b"Created"
            flow["response"]["content"] = b'{"id": 3, "name": "New User"}'
        elif method == b"PUT":
            flow["request"]["headers"].append([b"Content-Type", b"application/json"])
            flow["request"]["content"] = b'{"name": "Updated User"}'
            flow["response"]["content"] = b'{"id": 1, "name": "Updated User"}'
        elif method == b"DELETE":
            flow["response"]["status_code"] = 204
            flow["response"]["reason"] = b"No Content"
            flow["response"]["content"] = b""
            flow["response"]["headers"] = []
        elif path == b"/api/v1/users/1":
            flow["response"]["content"] = b'{"id": 1, "name": "Alice"}'
        elif path == b"/api/v1/products":
            flow["response"]["content"] = (
                b'{"products": [{"id": 1, "name": "Widget", "price": 9.99}]}'
            )

        flows.append(flow)

    _write_flows(FLOWS_DIR / "multiple.flow", flows)


def gen_corrupt():
    """Truncated tnetstring for error-handling tests."""
    valid = tnetstring.dumps(_base_flow())
    # Take first 50 bytes — enough to start parsing but not finish
    truncated = valid[:50]
    with open(FLOWS_DIR / "corrupt.flow", "wb") as f:
        f.write(truncated)


# ---------------------------------------------------------------------------
# HAR fixtures
# ---------------------------------------------------------------------------


def _har_entry(
    url="https://api.example.com/api/v1/users",
    method="GET",
    status=200,
    status_text="OK",
    req_headers=None,
    req_body=None,
    req_mime=None,
    resp_headers=None,
    resp_body=None,
    resp_mime="application/json",
    resp_encoding=None,
):
    """Build a single HAR entry."""
    now_iso = "2025-01-15T10:30:00.000Z"

    req_h = req_headers or [
        {"name": "Host", "value": "api.example.com"},
        {"name": "Accept", "value": "application/json"},
    ]
    resp_h = resp_headers or [{"name": "Content-Type", "value": resp_mime}]

    entry = {
        "startedDateTime": now_iso,
        "time": 100,
        "request": {
            "method": method,
            "url": url,
            "httpVersion": "HTTP/1.1",
            "cookies": [],
            "headers": req_h,
            "queryString": [],
            "headersSize": -1,
            "bodySize": len(req_body) if req_body else 0,
        },
        "response": {
            "status": status,
            "statusText": status_text,
            "httpVersion": "HTTP/1.1",
            "cookies": [],
            "headers": resp_h,
            "content": {
                "size": len(resp_body) if resp_body else 0,
                "mimeType": resp_mime,
            },
            "redirectURL": "",
            "headersSize": -1,
            "bodySize": len(resp_body) if resp_body else 0,
        },
        "cache": {},
        "timings": {
            "send": 1,
            "wait": 90,
            "receive": 9,
        },
    }

    if req_body:
        entry["request"]["postData"] = {
            "mimeType": req_mime or "application/json",
            "text": req_body,
        }

    if resp_body:
        if resp_encoding:
            entry["response"]["content"]["encoding"] = resp_encoding
        entry["response"]["content"]["text"] = resp_body

    return entry


def _har_wrapper(entries):
    return {
        "log": {
            "version": "1.2",
            "creator": {
                "name": "generate_fixtures.py",
                "version": "1.0",
            },
            "entries": entries,
        }
    }


def gen_har_simple():
    entry = _har_entry(
        resp_body='{"users": [{"id": 1, "name": "Alice"}]}',
    )
    har = _har_wrapper([entry])
    with open(HAR_DIR / "simple.har", "w") as f:
        json.dump(har, f, indent=2)


def gen_har_multi():
    entries = [
        _har_entry(
            url="https://api.example.com/api/v1/users",
            method="GET",
            resp_body='{"users": []}',
        ),
        _har_entry(
            url="https://api.example.com/api/v1/users",
            method="POST",
            status=201,
            status_text="Created",
            req_body='{"name": "Bob"}',
            req_mime="application/json",
            resp_body='{"id": 2, "name": "Bob"}',
        ),
        _har_entry(
            url="https://api.example.com/api/v1/products",
            method="GET",
            resp_body='{"products": []}',
        ),
    ]
    har = _har_wrapper(entries)
    with open(HAR_DIR / "multi.har", "w") as f:
        json.dump(har, f, indent=2)


def gen_har_base64():
    import base64

    binary_content = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"
    b64 = base64.b64encode(binary_content).decode("ascii")
    entry = _har_entry(
        url="https://api.example.com/api/v1/avatar/1.png",
        method="GET",
        resp_body=b64,
        resp_mime="image/png",
        resp_encoding="base64",
        resp_headers=[{"name": "Content-Type", "value": "image/png"}],
    )
    har = _har_wrapper([entry])
    with open(HAR_DIR / "base64_body.har", "w") as f:
        json.dump(har, f, indent=2)


# ---------------------------------------------------------------------------
# Golden OpenAPI YAML fixtures
# ---------------------------------------------------------------------------


def gen_expected_simple_get():
    spec = {
        "openapi": "3.0.3",
        "info": {
            "title": "api.example.com API",
            "version": "1.0.0",
        },
        "servers": [
            {"url": "https://api.example.com"},
        ],
        "paths": {
            "/api/v1/users": {
                "get": {
                    "summary": "GET /api/v1/users",
                    "responses": {
                        "200": {
                            "description": "OK",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "users": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "id": {"type": "integer"},
                                                        "name": {"type": "string"},
                                                    },
                                                },
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            },
        },
    }
    with open(EXPECTED_DIR / "simple_get.yaml", "w") as f:
        _yaml.dump(spec, f)


def gen_expected_post_json():
    spec = {
        "openapi": "3.0.3",
        "info": {
            "title": "api.example.com API",
            "version": "1.0.0",
        },
        "servers": [
            {"url": "https://api.example.com"},
        ],
        "paths": {
            "/api/v1/users": {
                "post": {
                    "summary": "POST /api/v1/users",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "name": {"type": "string"},
                                        "email": {"type": "string"},
                                    },
                                },
                            },
                        },
                    },
                    "responses": {
                        "201": {
                            "description": "Created",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "id": {"type": "integer"},
                                            "name": {"type": "string"},
                                            "email": {"type": "string"},
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            },
        },
    }
    with open(EXPECTED_DIR / "post_json.yaml", "w") as f:
        _yaml.dump(spec, f)


def gen_expected_multi():
    spec = {
        "openapi": "3.0.3",
        "info": {
            "title": "api.example.com API",
            "version": "1.0.0",
        },
        "servers": [
            {"url": "https://api.example.com"},
        ],
        "paths": {
            "/api/v1/users": {
                "get": {
                    "summary": "GET /api/v1/users",
                    "responses": {
                        "200": {
                            "description": "OK",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "users": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "id": {"type": "integer"},
                                                        "name": {"type": "string"},
                                                    },
                                                },
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
                "post": {
                    "summary": "POST /api/v1/users",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "name": {"type": "string"},
                                    },
                                },
                            },
                        },
                    },
                    "responses": {
                        "201": {
                            "description": "Created",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "id": {"type": "integer"},
                                            "name": {"type": "string"},
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            },
            "/api/v1/users/{id}": {
                "get": {
                    "summary": "GET /api/v1/users/{id}",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": True,
                            "schema": {"type": "string"},
                        },
                    ],
                    "responses": {
                        "200": {
                            "description": "OK",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "id": {"type": "integer"},
                                            "name": {"type": "string"},
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
                "put": {
                    "summary": "PUT /api/v1/users/{id}",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": True,
                            "schema": {"type": "string"},
                        },
                    ],
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "name": {"type": "string"},
                                    },
                                },
                            },
                        },
                    },
                    "responses": {
                        "200": {
                            "description": "OK",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "id": {"type": "integer"},
                                            "name": {"type": "string"},
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
                "delete": {
                    "summary": "DELETE /api/v1/users/{id}",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": True,
                            "schema": {"type": "string"},
                        },
                    ],
                    "responses": {
                        "204": {
                            "description": "No Content",
                        },
                    },
                },
            },
            "/api/v1/products": {
                "get": {
                    "summary": "GET /api/v1/products",
                    "responses": {
                        "200": {
                            "description": "OK",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "products": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "id": {"type": "integer"},
                                                        "name": {"type": "string"},
                                                        "price": {"type": "number"},
                                                    },
                                                },
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            },
        },
    }
    with open(EXPECTED_DIR / "multi.yaml", "w") as f:
        _yaml.dump(spec, f)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    ensure_dirs()

    print("Generating flow fixtures...")
    gen_simple_get()
    gen_post_json()
    gen_post_form()
    gen_multi_status()
    gen_no_response()
    gen_non_utf8()
    gen_multiple()
    gen_corrupt()
    print(f"  Created {len(list(FLOWS_DIR.glob('*.flow')))} .flow files")

    print("Generating HAR fixtures...")
    gen_har_simple()
    gen_har_multi()
    gen_har_base64()
    print(f"  Created {len(list(HAR_DIR.glob('*.har')))} .har files")

    print("Generating golden YAML fixtures...")
    gen_expected_simple_get()
    gen_expected_post_json()
    gen_expected_multi()
    print(f"  Created {len(list(EXPECTED_DIR.glob('*.yaml')))} .yaml files")

    print("Done.")


if __name__ == "__main__":
    main()
