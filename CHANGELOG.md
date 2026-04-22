# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.5](https://github.com/Arkptz/mitm2openapi/compare/v0.2.4...v0.2.5) - 2026-04-22

### Fixed

- *(builder)* skip requests with unknown HTTP methods instead of aliasing to GET

### Other

- *(builder)* verify unknown method is skipped and standard methods preserved

## [0.2.4](https://github.com/Arkptz/mitm2openapi/compare/v0.2.3...v0.2.4) - 2026-04-22

### Fixed

- *(params)* preserve multi-byte UTF-8 in urlencoding_decode

### Other

- *(params)* add UTF-8 roundtrip and overlong rejection cases
- regenerate demo.gif [skip ci]

## [0.2.3](https://github.com/Arkptz/mitm2openapi/compare/v0.2.2...v0.2.3) - 2026-04-22

### Fixed

- *(builder)* cap form-field count per request at 1000
- *(har)* validate schemes and status codes, log base64 failures, cap bodies
- *(reader)* validate port/status ranges, enforce strict UTF-8, and cap field sizes

### Other

- *(readme)* document per-field size and validation limits

## [0.2.2](https://github.com/Arkptz/mitm2openapi/compare/v0.2.1...v0.2.2) - 2026-04-22

### Added

- *(har)* add streaming HAR entry iterator

### Other

- *(readme)* mention HAR streaming in resource limits and supported formats
- *(har)* verify streaming does not materialize all entries
- *(reader)* switch HAR dispatch to streaming iterator
- regenerate demo.gif [skip ci]

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
