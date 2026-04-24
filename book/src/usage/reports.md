# Processing reports

Pass `--report <PATH>` to either `discover` or `generate` to write a JSON processing
summary. This is useful for CI pipelines that need structured data instead of log scraping.

## Usage

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --report report.json
```

## Report schema

```json
{
  "report_version": 1,
  "tool_version": "0.5.1",
  "input": {
    "path": "capture.flow",
    "format": "Auto",
    "size_bytes": 102400
  },
  "result": {
    "flows_read": 150,
    "flows_emitted": 148,
    "paths_in_spec": 12
  },
  "events": {
    "parse_error": {
      "TNetString parse error at byte 98304: unexpected end of input": 1
    }
  }
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `report_version` | integer | Schema version (currently `1`) |
| `tool_version` | string | `mitm2openapi` version that produced the report |
| `input.path` | string | Input file path |
| `input.format` | string | Detected or specified format (`Auto`, `Mitmproxy`, `Har`) |
| `input.size_bytes` | integer | Input file size in bytes |
| `result.flows_read` | integer | Total flows/entries parsed from input |
| `result.flows_emitted` | integer | Flows that passed all filters and were processed |
| `result.paths_in_spec` | integer | Unique paths in the output (for `generate`) |
| `events` | object | Map of event categories to message counts |

### Event categories

| Category | Meaning | Status |
|----------|---------|--------|
| `parse_error` | Corrupt data encountered (tnetstring errors, malformed HAR entries) | Populated |
| `cap_fired` | A resource limit was triggered (body too large, depth exceeded) | Reserved — not yet populated at runtime |
| `rejected` | A flow was skipped (invalid UTF-8, unsupported scheme, bad port/status) | Reserved — not yet populated at runtime |

The `cap_fired` and `rejected` categories are present in the report schema and will be
connected to the reader pipelines in a future release. Currently, only `parse_error`
events are counted.

## CI integration

Parse the report in CI to make decisions based on processing quality:

```bash
mitm2openapi generate \
  -i capture.flow \
  -t templates.yaml \
  -o openapi.yaml \
  -p "https://api.example.com" \
  --report report.json

# Check if any events occurred
if jq -e '.events | length > 0' report.json > /dev/null 2>&1; then
  echo "Warning: processing had events"
  jq '.events' report.json
fi
```

## Report with strict mode

The report is written even when `--strict` causes a non-zero exit code. This lets you
capture full diagnostics while still failing the CI job:

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --strict \
  --report report.json \
  || { jq '.' report.json; exit 1; }
```
