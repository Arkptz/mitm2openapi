# Resource limits

<!-- toc -->

To prevent denial-of-service when processing untrusted captures, `mitm2openapi` enforces
several configurable and fixed limits.

## Configurable limits

These limits can be adjusted via CLI flags:

| Flag | Default | Purpose |
|------|---------|---------|
| `--max-input-size` | 2 GiB | Reject files larger than this before reading |
| `--max-payload-size` | 256 MiB | Cap on individual tnetstring payload allocation |
| `--max-depth` | 256 | Recursion depth limit for nested tnetstring structures |
| `--max-body-size` | 64 MiB | Maximum request/response body considered during schema inference |
| `--allow-symlinks` | off | By default, symlinked inputs are rejected |

### Adjusting limits

Increase `--max-input-size` if you work with captures larger than 2 GiB:

```bash
mitm2openapi discover \
  -i large-capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --max-input-size 8GiB
```

Size suffixes are supported: `KiB`, `MiB`, `GiB`.

The other limits rarely need tuning. The defaults are designed to handle real-world
captures while rejecting pathological inputs.

### Symlink rejection

By default, symlinked input files are rejected to prevent path-traversal attacks on shared
CI runners. If you need to process a symlinked file:

```bash
mitm2openapi discover \
  -i /path/to/symlinked-capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --allow-symlinks
```

## Fixed per-field limits

These limits are applied unconditionally and cannot be changed via CLI flags:

| Field | Cap | Behaviour when exceeded |
|-------|-----|------------------------|
| Header name | 8 KiB | Header dropped (other headers still processed) |
| Header value | 64 KiB | Value truncated to cap |
| Form fields per request | 1,000 | Excess fields ignored |
| URL scheme | `http` / `https` only | Non-HTTP flows silently skipped |
| Port number | 1 -- 65,535 | Out-of-range port drops the request |
| HTTP status code | 100 -- 599 | Invalid codes treated as no response |

## UTF-8 validation

Identity fields (scheme, host, path, method, header names) require valid UTF-8. Flows
with non-UTF-8 identity bytes are skipped to prevent data aliasing through
replacement-character collisions.

Control characters (`0x00`--`0x1F`, `0x7F`) in paths are stripped automatically.

## Streaming and memory

Both mitmproxy flow files and HAR files are processed incrementally. Memory usage stays
bounded regardless of input size — there is no need to load the entire capture into memory.

Peak RSS is proportional to the size of the **largest single flow** in the capture, not the
total file size. For typical captures, expect 5--15 MB of memory usage.

## When limits fire

When a limit is exceeded:

- A `warn`-level log message is emitted with details
- The affected flow or field is skipped/truncated
- Processing continues with the remaining data

Use [strict mode](./strict-mode.md) to treat these warnings as errors, or
[processing reports](./reports.md) to capture them as structured data.
