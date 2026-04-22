# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/Arkptz/mitm2openapi/compare/v0.2.0...v0.2.1) - 2026-04-22

### Added

- *(reader)* add stream_mitmproxy_file and stream_mitmproxy_dir
- *(tnetstring)* add streaming iterator TNetStringIter

### Other

- *(readme)* document resource-limit flags and streaming behavior
- *(main)* switch discover and generate to streaming pipeline
- *(path_matching)* cache compiled regexes in CompiledTemplates
- *(builder)* add discover_paths_streaming variant
- regenerate demo.gif [skip ci]

## [0.2.0](https://github.com/Arkptz/mitm2openapi/compare/v0.1.2...v0.2.0) - 2026-04-22

### Added

- *(path_matching)* validate path parameter identifiers
- *(cli)* expose --max-input-size, --max-payload-size, --max-depth, --max-body-size, --allow-symlinks
- *(reader)* reject symlinks, non-regular files, and oversized inputs
- *(schema)* enforce 64-level JSON recursion depth limit
- *(tnetstring)* enforce 256-level recursion depth limit
- *(tnetstring)* cap payload size at 256 MiB
- *(error)* add typed variants for parse and input limits

### Fixed

- *(test)* gate symlink and FIFO tests behind cfg(unix)

### Other

- update Cargo.lock for globset dependency
- *(security)* cover symlink, FIFO, and oversize input rejection
- *(har)* bound format-detection read to 4 KiB
- *(builder)* replace custom glob matcher with globset

## [0.1.2](https://github.com/Arkptz/mitm2openapi/compare/v0.1.1...v0.1.2) - 2026-04-22

### Other

- add tests/fixtures/poc placeholder directory (P0.2)
- regenerate demo.gif [skip ci]

## [0.1.1](https://github.com/Arkptz/mitm2openapi/compare/v0.1.0...v0.1.1) - 2026-04-20

### Other

- *(readme)* add Why? section explaining the Python-vs-Rust trade-off
- *(deps)* bump assert_cmd from 2.2.0 to 2.2.1 in the all-cargo group ([#7](https://github.com/Arkptz/mitm2openapi/pull/7))
- regenerate demo.gif [skip ci]

## [0.1.0] - TBD

Initial release.
