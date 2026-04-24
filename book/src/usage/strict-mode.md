# Strict mode

Pass `--strict` to either `discover` or `generate` to treat warning-level events as
hard failures. The process exits with code **2** if the processing report records any
counted events.

Currently, the only event counter populated at runtime is `parse_error` — triggered when
flows cannot be deserialized (corrupt tnetstring data, malformed HAR JSON). The
`cap_fired` and `rejected` counters exist in the report schema but are not yet wired to
the reader pipelines; they will be connected in a future release.

In practice, `--strict` today catches:

- Parse errors during flow deserialization (tnetstring or HAR)
- Errors counted by the streaming iterator wrapper in `discover` mode

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

Without the flag, parse errors are logged at `warn` level and processing continues with
exit code 0. Affected flows are skipped, but the output file is still produced. Other
warning-level events (cap fires, scheme rejections, etc.) are always logged but do not
currently increment the report counters that `--strict` checks.

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
