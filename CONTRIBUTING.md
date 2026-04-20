# Contributing — Local Testing Guide

This document covers how to run the three test tracks locally.

## Prerequisites

| Tool                     | Required for            | Install                                                                |
| ------------------------ | ----------------------- | ---------------------------------------------------------------------- |
| Rust toolchain           | Build + unit tests      | [rustup.rs](https://rustup.rs)                                         |
| Docker + Compose         | All integration tests   | [docs.docker.com](https://docs.docker.com/get-docker/)                 |
| `oasdiff`                | Level 1 diff validation | `go install github.com/tufin/oasdiff@latest` or `brew install oasdiff` |
| Node.js + npm            | Level 2                 | [nodejs.org](https://nodejs.org)                                       |
| Playwright               | Level 2                 | `npx playwright install --with-deps chromium`                          |
| VHS                      | Demo GIF                | `brew install vhs` or [charm apt repo](https://charm.sh)               |
| ffmpeg, gifski, gifsicle | Demo GIF optimization   | System package manager                                                 |

## Build

```bash
cargo build --release
# Binary: target/release/mitm2openapi
```

## Unit Tests

```bash
cargo test
```

## Level 1 — Petstore Golden Test (~2 min)

Full pipeline (compose up, seed, discover, generate, diff, teardown):

```bash
tests/integration/level1/run.sh
```

Strict mode (`--fail-on WARN` instead of `BREAKING`):

```bash
tests/integration/level1/run.sh --strict
```

Manual step-by-step:

```bash
cd tests/integration/level1
docker compose up -d
# Wait for petstore healthcheck...
../../ci/petstore/seed.sh
# Run mitm2openapi discover/generate against the proxy
# ...
docker compose down -v
```

> **Gotcha**: The seed script sends requests through the mitmproxy proxy to `petstore:8080` (Docker service name), not `localhost`. This is intentional — traffic must flow through the proxy to be captured.

## Level 2 — crAPI + Playwright (~8 min)

```bash
cd tests/integration/level2

# Start crAPI stack (identity + community + workshop + web + mongo + postgres + mailhog + mitmproxy)
make up
# No seed needed — crAPI auto-seeds on first boot

# Run Playwright scenarios
npm install
npx playwright install --with-deps chromium
npx playwright test

# Cleanup
make down
```

> **Port conflict**: Level 1 and Level 2 both use port 8080 (for different services). Do not run both stacks simultaneously.

## Demo GIF (Phase 2 terminal recording)

```bash
cd ci/demo
make phase2        # VHS recording
make gif           # gifski + gifsicle optimization
make clean         # remove outputs
```

## Ports Reference

| Stack   | Service    | Port |
| ------- | ---------- | ---- |
| Level 1 | Petstore   | 8080 |
| Level 1 | mitmproxy  | 8081 |
| Level 2 | crAPI web  | 8888 |
| Level 2 | mailhog    | 8025 |
| Level 2 | mitmproxy  | 8080 |
| Demo    | Swagger UI | 8088 |

## Cleanup

All compose stacks use `docker compose down -v` to remove containers and volumes.

## CI Workflows

| Workflow                 | Trigger                                        | Notes                                     |
| ------------------------ | ---------------------------------------------- | ----------------------------------------- |
| `integration-level1.yml` | Every PR                                       | Naive (required) + strict (informational) |
| `integration-level2.yml` | Nightly + manual dispatch                      | Full crAPI + Playwright                   |
| `demo-gif.yml`           | Push to main (path-filtered) + manual dispatch | Terminal recording                        |
