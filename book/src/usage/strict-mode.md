# Strict mode

Pass `--strict` to either `discover` or `generate` to treat any warning-level event as a
hard failure. The process exits with code **2** if any of these occur:

- A resource cap fired (input too large, payload too large, depth exceeded)
- A flow was rejected (invalid UTF-8, unsupported scheme, malformed data)
- A parse error was encountered (corrupt tnetstring, malformed HAR)

## Usage

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --strict
```

```bash
mitm2openapi generate \
  -i capture.flow \
  -t templates.yaml \
  -o openapi.yaml \
  -p "https://api.example.com" \
  --strict
```

## CI usage pattern

Strict mode is designed for CI gates where silent degradation is unacceptable:

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --strict \
  || { echo "FAIL: corrupt or over-limit flows detected"; exit 1; }
```

## Without `--strict`

Without the flag, the same conditions are logged at `warn` level and processing continues
with exit code 0. Affected flows or fields are skipped/truncated, but the output file is
still produced.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success (no warnings, or `--strict` not set) |
| 1 | Fatal error (I/O failure, missing required arguments) |
| 2 | Strict mode violation (warnings detected with `--strict`) |

## Combining with reports

For CI pipelines that need both strict enforcement and structured diagnostics:

```bash
mitm2openapi generate \
  -i capture.flow \
  -t templates.yaml \
  -o openapi.yaml \
  -p "https://api.example.com" \
  --strict \
  --report report.json
```

The [report](./reports.md) is written even when `--strict` causes a non-zero exit, capturing
the full details of what went wrong.
