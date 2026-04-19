![Nightly Integration](https://github.com/arkptz/mitmproxy2swagger-rs/actions/workflows/integration-level2.yml/badge.svg)

<img src="docs/demo.gif" alt="Demo: capture → generate → browse">

# mitm2openapi

Convert mitmproxy flow dumps and HAR files to OpenAPI 3.0 specifications.

A Rust rewrite of [mitmproxy2swagger](https://github.com/aluber/mitmproxy2swagger) — faster, standalone binary, no Python runtime required.

## Installation

### From binary releases

Download a pre-built binary from [GitHub Releases](https://github.com/arkptz/mitmproxy2swagger-rs/releases).

### From source

```bash
cargo install --git https://github.com/arkptz/mitmproxy2swagger-rs
```

## Quick Start

```bash
# 1. Capture traffic with mitmproxy
mitmdump -w capture.flow

# 2. Discover API endpoints
mitm2openapi discover -i capture.flow -o templates.yaml -p "https://api.example.com"

# 3. Edit templates.yaml — remove 'ignore:' prefix from paths you want to include

# 4. Generate OpenAPI spec
mitm2openapi generate -i capture.flow -t templates.yaml -o openapi.yaml -p "https://api.example.com"
```

## CLI Reference

### `discover`

Scan captured traffic and produce a templates file listing all observed endpoints.

```
mitm2openapi discover [OPTIONS] -i <INPUT> -o <OUTPUT> -p <PREFIX>
```

| Option | Description |
|--------|-------------|
| `-i, --input <PATH>` | Input file (flow dump or HAR) |
| `-o, --output <PATH>` | Output YAML templates file |
| `-p, --prefix <URL>` | API prefix URL to filter requests |
| `--format <FORMAT>` | Input format: `auto`, `har`, `mitmproxy` (default: `auto`) |

### `generate`

Generate an OpenAPI 3.0 spec from captured traffic using a curated templates file.

```
mitm2openapi generate [OPTIONS] -i <INPUT> -t <TEMPLATES> -o <OUTPUT> -p <PREFIX>
```

| Option | Description |
|--------|-------------|
| `-i, --input <PATH>` | Input file (flow dump or HAR) |
| `-t, --templates <PATH>` | Templates YAML file (from `discover`) |
| `-o, --output <PATH>` | Output OpenAPI YAML file |
| `-p, --prefix <URL>` | API prefix URL |
| `--format <FORMAT>` | Input format: `auto`, `har`, `mitmproxy` (default: `auto`) |
| `--param-regex <REGEX>` | Custom regex for parameter detection |
| `--openapi-title <TITLE>` | Custom title for the spec |
| `--openapi-version <VER>` | Custom spec version (default: `1.0.0`) |
| `--exclude-headers <LIST>` | Comma-separated headers to exclude |
| `--exclude-cookies <LIST>` | Comma-separated cookies to exclude |
| `--include-headers` | Include headers in the spec |
| `--ignore-images` | Ignore image content types |
| `--suppress-params` | Suppress parameter suggestions |
| `--tags-overrides <JSON>` | JSON string for tag overrides |

## Supported Formats

| Format | Versions | Extension |
|--------|----------|-----------|
| mitmproxy flow dumps | v19, v20, v21 | `.flow` |
| HAR (HTTP Archive) | 1.2 | `.har` |

Format is auto-detected from file content. Use `--format` to override.

## Migration from Python mitmproxy2swagger

| Python (`mitmproxy2swagger`) | Rust (`mitm2openapi`) |
|-----|-----|
| `pip install mitmproxy2swagger` | Single binary, no runtime |
| `mitmproxy2swagger -i <file> -o <spec> -p <prefix>` | Two-step: `discover` then `generate` |
| Edits spec file in-place | Separate templates file for curation |
| Requires Python 3.x + mitmproxy | Standalone binary |
| Supports mitmproxy only | Supports mitmproxy flow dumps + HAR |

### Key differences

- **Two-step workflow**: `discover` produces a templates file; you curate it; `generate` produces the final spec. This separates endpoint selection from spec generation.
- **Templates file**: Discovered endpoints are prefixed with `ignore:`. Remove the prefix to include an endpoint. This replaces editing the output spec directly.
- **No Python dependency**: Ships as a single static binary for Linux, macOS, and Windows.
- **HAR support**: Process HAR exports from browser DevTools or other HTTP tools.

## License

MIT
