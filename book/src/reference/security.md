# Security model

<!-- toc -->

`mitm2openapi` processes untrusted binary input (traffic captures from unknown sources).
The security model is designed to prevent denial-of-service, data corruption, and
information leakage when handling adversarial input.

## Threat model

The primary threat is a **malicious capture file** — a `.flow` or `.har` file crafted to
exploit the parser. Scenarios include:

- CI pipelines processing captures from untrusted contributors
- Shared analysis servers where multiple users submit captures
- Automated pipelines where the capture source is not fully controlled

## Input validation layers

### File-level checks

Before reading any content:

1. **File type** — only regular files are accepted. Symlinks, FIFOs, device files, and
   directories are rejected unless `--allow-symlinks` is explicitly set.
2. **File size** — files exceeding `--max-input-size` (default 2 GiB) are rejected before
   any bytes are read.
3. **TOCTOU caveat** — file metadata is checked via the path before reading to reject
   symlinks, non-regular files, and oversized inputs. There is a small TOCTOU window
   between the metadata check and the file open; mitigation via fd-based recheck after
   open is a future enhancement.

### Parser-level caps

During parsing:

| Cap | Default | Purpose |
|-----|---------|---------|
| Payload size | 256 MiB | Prevents OOM from oversized tnetstring values |
| Nesting depth | 256 | Prevents stack overflow from deeply nested structures |
| JSON depth | 64 | Prevents stack overflow in schema inference |
| Body size | 64 MiB | Limits memory for individual request/response bodies |

These caps trigger `warn`-level events and skip the affected data. Use `--strict` to
treat them as hard errors.

### Field-level validation

For every flow:

- **Scheme whitelist** — only `http` and `https` are accepted. Other schemes (e.g.,
  `javascript:`, `data:`) are silently skipped.
- **UTF-8 strictness** — identity fields (method, scheme, host, path, header names) must be
  valid UTF-8. Invalid bytes cause the flow to be skipped, preventing data aliasing through
  replacement-character collisions.
- **Port range** — port numbers must be 1--65,535. Out-of-range values drop the request.
- **Status code range** — HTTP status codes must be 100--599.
- **Control character stripping** — `0x00`--`0x1F` and `0x7F` in URL paths are removed.
- **Header caps** — header names over 8 KiB are dropped; values over 64 KiB are truncated.
- **Form field count** — at most 1,000 form fields per request are processed.

### Output safety

- **Atomic writes** — output files are written via a temporary file and renamed. If the write
  fails (disk full, permission denied), the target path is left untouched.
- **No resync on corruption** — when the tnetstring parser encounters corrupt data, it halts
  immediately. It does not scan forward looking for the next valid frame, because binary
  payloads can contain bytes that look like valid length prefixes.

## Streaming architecture

Both mitmproxy and HAR inputs are processed incrementally. At no point is the entire capture
loaded into memory. This bounds peak RSS to the size of the largest single flow, regardless
of total file size.

## Glob pattern safety

The `--exclude-patterns` and `--include-patterns` flags use the
[globset](https://docs.rs/globset) crate, which compiles patterns into a DFA. This eliminates
exponential backtracking that was possible with the original recursive glob matcher.

## Recommendations

For processing untrusted captures:

1. Do not use `--allow-symlinks` unless you control the filesystem
2. Keep `--max-input-size` at the default (2 GiB) or lower
3. Run with `--strict` to fail fast on any anomaly
4. Use `--report` to capture processing diagnostics for audit trails
5. Run in a sandboxed environment (container, VM) when processing captures from unknown sources

## Related

- [Resource limits](../usage/resource-limits.md) — configuring the caps
- [Strict mode](../usage/strict-mode.md) — CI enforcement
- [Diagnostics](./diagnostics.md) — interpreting warnings and errors
