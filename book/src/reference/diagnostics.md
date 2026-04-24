# Diagnostics

<!-- toc -->

`mitm2openapi` uses structured logging to report issues during processing. This chapter
covers how to interpret warnings, errors, and the structured report output.

## Log levels

Control verbosity with the `RUST_LOG` environment variable:

```bash
# Default: warnings only
mitm2openapi discover -i capture.flow -o templates.yaml -p "https://api.example.com"

# More detail
RUST_LOG=info mitm2openapi discover -i capture.flow -o templates.yaml -p "https://api.example.com"

# Full debug output
RUST_LOG=debug mitm2openapi discover -i capture.flow -o templates.yaml -p "https://api.example.com"
```

## Common warnings

### Parse errors (tnetstring)

```
WARN TNetString parse error at byte 98304: unexpected end of input (148 flows parsed successfully)
```

This means the mitmproxy flow file contains corrupt data starting at byte 98,304. The
parser halted and all 148 flows parsed before the corruption are still processed.

**No resync is attempted.** Binary payloads can contain bytes that mimic valid tnetstring
length prefixes, so scanning forward would produce phantom flows with fabricated data.

**What to do:**
- If the file was truncated during transfer, re-capture or re-download
- The 148 successfully parsed flows are still usable
- Use `--report` to capture the exact byte offset for debugging

### Cap-fired events

```
WARN body size 68157440 exceeds cap 67108864, truncating
WARN header name exceeds 8192 bytes, dropping
WARN form field count 1247 exceeds cap 1000, ignoring excess
```

These indicate that a specific field in a flow exceeded the built-in or configured limit.
The affected field is truncated or dropped, but processing continues.

**What to do:**
- Usually safe to ignore — the caps exist to prevent abuse, not normal traffic
- If you need the full data, increase the relevant `--max-*` flag
- Use `--strict` to fail on these if you need guaranteed completeness

### Flow rejection events

```
WARN skipping flow: scheme "javascript" not in whitelist [http, https]
WARN skipping flow: invalid UTF-8 in host field
WARN skipping flow: port 0 out of valid range 1-65535
```

These mean an entire flow was skipped because it failed validation.

**What to do:**
- Non-HTTP flows (WebSocket upgrades, CONNECT tunnels) are expected to be skipped
- UTF-8 errors suggest the capture contains binary protocol data, not HTTP traffic
- Invalid port/status usually indicates corrupt flow data

## Structured reports

For machine-readable diagnostics, use `--report`:

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --report report.json
```

See [processing reports](../usage/reports.md) for the full JSON schema.

### Event categories in reports

| Category | Examples |
|----------|---------|
| `parse_error` | Tnetstring corruption, HAR JSON syntax errors |
| `cap_fired` | Body too large, depth exceeded, form field count exceeded |
| `rejected` | Invalid scheme, non-UTF-8 identity fields, bad port/status |

### Using reports in CI

```bash
# Fail if any parse errors occurred
if jq -e '.events.parse_error | length > 0' report.json > /dev/null 2>&1; then
  echo "Parse errors detected"
  exit 1
fi

# Check flows-read vs flows-emitted ratio
RATIO=$(jq '.result.flows_emitted / .result.flows_read' report.json)
if (( $(echo "$RATIO < 0.9" | bc -l) )); then
  echo "Warning: more than 10% of flows were dropped"
fi
```

## Strict mode interaction

With `--strict`, any warning-level event causes exit code 2. This converts the
"informational" diagnostics above into hard failures:

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --strict \
  --report report.json

# Exit code 2 if ANY warning was emitted
# report.json still written for post-mortem
```

See [strict mode](../usage/strict-mode.md) for details.
