# mitmproxy flow dumps

`mitm2openapi` reads mitmproxy's native binary flow format. This is the recommended input
format — it captures the richest data and is produced directly by `mitmdump` and `mitmweb`.

## Supported versions

| Flow format version | mitmproxy version | Status |
|---|---|---|
| v19 | mitmproxy 8.x | Supported |
| v20 | mitmproxy 9.x | Supported |
| v21 | mitmproxy 10.x | Supported |

The flow format is auto-detected from file content. No version flag is needed.

## How flow files work

Flow files use the [tnetstring](https://tnetstrings.info/) serialization format. Each flow
is a sequence of key-value pairs representing a complete HTTP request-response cycle.

A typical flow contains:

- **Request**: method, URL (scheme, host, port, path), headers, body
- **Response**: status code, headers, body
- **Metadata**: timestamps, flow ID, client/server addresses

`mitm2openapi` extracts the request and response data relevant to OpenAPI spec generation
and discards metadata.

## Capturing flow files

```bash
# Record all traffic through the proxy
mitmdump -w capture.flow

# Record only traffic to a specific host
mitmdump -w capture.flow --set flow_detail=0 \
  --set save_stream_filter='~d api.example.com'
```

See [capturing traffic](../getting-started/capturing.md) for full setup instructions.

## Directory input

If you pass a directory path to `-i`, `mitm2openapi` reads all `.flow` files in that
directory (non-recursive). This is useful when you have traffic split across multiple
capture sessions.

## Known limitations

- **No WebSocket frames** — WebSocket upgrade requests are captured, but frame-level data
  is not used for spec generation
- **No gRPC** — binary protocol buffers inside HTTP/2 frames are not decoded
- **Corrupt files** — when the tnetstring parser hits corruption, it stops and reports the
  byte offset. No resync is attempted because binary payloads can contain bytes that mimic
  valid tnetstring length prefixes. See [diagnostics](../reference/diagnostics.md) for details.
- **Large payloads** — individual tnetstring payloads are capped at 256 MiB by default
  (adjustable via `--max-payload-size`)
