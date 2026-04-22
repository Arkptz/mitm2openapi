<div align="center">

# mitm2openapi

**Convert mitmproxy flow dumps and HAR files to OpenAPI 3.0 — fast, single binary, no Python.**

A Rust rewrite of [mitmproxy2swagger](https://github.com/alufers/mitmproxy2swagger).

[![CI](https://github.com/Arkptz/mitm2openapi/actions/workflows/ci.yml/badge.svg)](https://github.com/Arkptz/mitm2openapi/actions/workflows/ci.yml)
[![Nightly Integration](https://github.com/Arkptz/mitm2openapi/actions/workflows/integration-level2.yml/badge.svg)](https://github.com/Arkptz/mitm2openapi/actions/workflows/integration-level2.yml)
[![Crates.io](https://img.shields.io/crates/v/mitm2openapi.svg)](https://crates.io/crates/mitm2openapi)
[![Downloads](https://img.shields.io/crates/d/mitm2openapi.svg)](https://crates.io/crates/mitm2openapi)
[![docs.rs](https://img.shields.io/docsrs/mitm2openapi)](https://docs.rs/mitm2openapi)
[![docs](https://img.shields.io/badge/docs-arkptz.github.io-blue)](https://arkptz.github.io/mitm2openapi/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

<img src="docs/demo.gif" alt="Demo: capture → discover → generate → browse Swagger UI" width="720">

</div>

## Why?

[mitmproxy2swagger](https://github.com/alufers/mitmproxy2swagger) (the Python original by [@alufers](https://github.com/alufers)) works well, but requires Python, `pip`, and `mitmproxy` installed in the environment. For CI pipelines, slim Docker images, security audits, and one-off usage that's friction.

`mitm2openapi` ships as a single ~5 MB static binary — drop it into any environment, no runtime, no `venv`, no `pip install`. Same OpenAPI 3.0 output as the original, plus first-class HAR support and glob-based filters for fully unattended pipelines.

Credit to [@alufers](https://github.com/alufers) for the original tool that pioneered this workflow.

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

```bash
cargo install mitm2openapi
```

Or download a pre-built binary from [GitHub Releases](https://github.com/Arkptz/mitm2openapi/releases).

## Quick start

```bash
# 1. Capture traffic with mitmproxy
mitmdump -w capture.flow

# 2. Discover API endpoints
mitm2openapi discover -i capture.flow -o templates.yaml -p "https://api.example.com"

# 3. Edit templates.yaml — remove 'ignore:' prefix from paths you want to include

# 4. Generate OpenAPI spec
mitm2openapi generate -i capture.flow -t templates.yaml -o openapi.yaml -p "https://api.example.com"
```

## Documentation

Full documentation at **[arkptz.github.io/mitm2openapi](https://arkptz.github.io/mitm2openapi/)** — covers installation, traffic capture setup, the full discover → curate → generate pipeline, CLI reference, resource limits, filtering, strict mode, format details, benchmarks, and security model.

## Benchmarks

Automated CI benchmarks run weekly against the Python original. See [docs/benchmarks.md](docs/benchmarks.md) for the latest comparison on a ~80 MB synthetic capture.

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for local testing setup (unit tests, Petstore golden test, crAPI integration, demo GIF pipeline).

## License

MIT
