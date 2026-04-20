<div align="center">

# mitm2openapi

**Convert mitmproxy flow dumps and HAR files to OpenAPI 3.0 — fast, single binary, no Python.**

A Rust rewrite of [mitmproxy2swagger](https://github.com/alufers/mitmproxy2swagger).

[![CI](https://github.com/Arkptz/mitm2openapi/actions/workflows/ci.yml/badge.svg)](https://github.com/Arkptz/mitm2openapi/actions/workflows/ci.yml)
[![Nightly Integration](https://github.com/Arkptz/mitm2openapi/actions/workflows/integration-level2.yml/badge.svg)](https://github.com/Arkptz/mitm2openapi/actions/workflows/integration-level2.yml)
[![Crates.io](https://img.shields.io/crates/v/mitm2openapi.svg)](https://crates.io/crates/mitm2openapi)
[![Downloads](https://img.shields.io/crates/d/mitm2openapi.svg)](https://crates.io/crates/mitm2openapi)
[![docs.rs](https://img.shields.io/docsrs/mitm2openapi)](https://docs.rs/mitm2openapi)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

<img src="docs/demo.gif" alt="Demo: capture → discover → generate → browse Swagger UI" width="720">

</div>

## Features

- **Fast** — pure Rust, single-threaded, processes captures in milliseconds
- **Single static binary** — no Python, no venv, no pip, no runtime dependencies
- **Two-format support** — mitmproxy flow dumps (v19/v20/v21) and HAR 1.2
- **Two-step workflow** — `discover` finds endpoints, you curate, `generate` emits clean OpenAPI 3.0
- **Glob filters** — `--exclude-patterns` and `--include-patterns` for automated pipelines
- **Error recovery** — skips corrupt flows, continues processing
- **Auto-detection** — heuristic format detection from file content
- **Battle-tested** — integration tests against Swagger Petstore and OWASP crAPI with `oasdiff` verification
- **Cross-platform** — Linux, macOS, Windows pre-built binaries

## Installation

### From binary releases

Download a pre-built binary from [GitHub Releases](https://github.com/Arkptz/mitm2openapi/releases).

### From source

```bash
cargo install --git https://github.com/Arkptz/mitm2openapi
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

### Skip the manual edit

If you know which paths you care about up front, use `--exclude-patterns`
and `--include-patterns` to let `discover` do the curation:

```bash
mitm2openapi discover \
  -i capture.flow -o templates.yaml -p "https://api.example.com" \
  --exclude-patterns '/static/**,/images/**,*.css,*.js,*.svg' \
  --include-patterns '/api/**,/v2/**'

mitm2openapi generate \
  -i capture.flow -t templates.yaml -o openapi.yaml -p "https://api.example.com"
```

Paths matching `--include-patterns` are auto-activated (emitted without
the `ignore:` prefix). Paths matching `--exclude-patterns` are dropped
entirely. Everything else still gets `ignore:` for manual review.

<details>
<summary><strong>CLI Reference</strong> (click to expand)</summary>

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
| `--exclude-patterns <GLOBS>` | Comma-separated globs; matching paths are dropped entirely. `*` = single segment, `**` = any subtree. E.g. `/static/**,*.css` |
| `--include-patterns <GLOBS>` | Comma-separated globs; matching paths are emitted without `ignore:` (auto-activated for `generate`) |

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

</details>

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

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for local testing setup (unit tests, Petstore golden test, crAPI integration, demo GIF pipeline).

## License

MIT
