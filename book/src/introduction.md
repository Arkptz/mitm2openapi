# Introduction

**mitm2openapi** converts [mitmproxy](https://mitmproxy.org/) flow dumps and HAR files into
[OpenAPI 3.0](https://spec.openapis.org/oas/v3.0.3) specifications. It ships as a single
static binary — no Python, no virtual environment, no runtime dependencies.

It is a Rust rewrite of [mitmproxy2swagger](https://github.com/alufers/mitmproxy2swagger) by
[@alufers](https://github.com/alufers), who pioneered the "capture traffic, extract API spec"
workflow. Credit to the original project for the idea and reference implementation.

## Why?

The Python original works well but requires Python, `pip`, and `mitmproxy` installed in the
environment. For CI pipelines, slim Docker images, security audits, and one-off usage, that
dependency chain is friction.

`mitm2openapi` ships as a single ~5 MB static binary. Drop it into any environment and run.
Same OpenAPI 3.0 output, plus first-class HAR support and glob-based filters for fully
unattended pipelines.

## Features

- **Fast** — pure Rust, ~17× faster than the Python original ([benchmarks](./reference/benchmarks.md))
- **Single static binary** — no Python, no venv, no pip, no runtime dependencies
- **Two-format support** — mitmproxy flow dumps (v19/v20/v21) and HAR 1.2
- **Two-step workflow** — `discover` finds endpoints, you curate, `generate` emits OpenAPI 3.0
- **Glob filters** — `--exclude-patterns` and `--include-patterns` for automated pipelines
- **Error recovery** — skips corrupt flows, continues processing
- **Auto-detection** — heuristic format detection from file content
- **Resource limits** — configurable caps prevent denial-of-service on untrusted input
- **Strict mode** — treat warnings as errors for CI gates
- **Structured reports** — `--report` outputs machine-readable JSON processing summaries
- **Battle-tested** — integration tests against Swagger Petstore and OWASP crAPI
- **Cross-platform** — Linux, macOS, Windows pre-built binaries

## How it works

The tool uses a two-step workflow:

1. **Discover** — scan captured traffic and list all observed API endpoints
2. **Curate** — review the list and select which endpoints to include
3. **Generate** — produce a clean OpenAPI 3.0 spec from the selected endpoints

This separates endpoint selection from spec generation, giving you full control over
what ends up in the final spec.

## Next steps

- [Install mitm2openapi](./getting-started/installation.md)
- [Run through the quick start](./getting-started/quick-start.md)
- [Learn about the full pipeline](./usage/pipeline.md)
