# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1](https://github.com/Arkptz/mitm2openapi/compare/v0.5.0...v0.5.1) - 2026-04-22

### Other

- *(bench)* refresh benchmark results
- *(bench)* drop small fixture tier
- *(readme)* add benchmarks section linking to automated results
- *(bench)* seed benchmarks.md with methodology and placeholders
- regenerate demo.gif [skip ci]

## [0.5.0](https://github.com/Arkptz/mitm2openapi/compare/v0.4.1...v0.5.0) - 2026-04-22

### Added

- *(cli)* add --strict flag to escalate warnings to errors

### Other

- *(readme)* document --strict flag and benchmark CI
- *(strict)* verify strict mode exit codes

## [0.4.1](https://github.com/Arkptz/mitm2openapi/compare/v0.4.0...v0.4.1) - 2026-04-22

### Fixed

- *(builder)* use .get() in dedup_schema_variants to satisfy indexing_slicing lint
- *(reader)* warn on skipped directory entries and malformed overrides
- *(schema)* union array element schemas and tighten dict heuristic

### Other

- *(lint)* deny clippy::indexing_slicing at crate level
- extract is_numeric_string and is_uuid to shared module
- *(output)* lazy-init regex via LazyLock
- *(error)* replace guarded unwrap sites with pattern matching

## [0.4.0](https://github.com/Arkptz/mitm2openapi/compare/v0.3.0...v0.4.0) - 2026-04-22

### Added

- feat!(builder): merge response schemas per status code
- feat!(cli): remove unused --param-regex flag

### Other

- *(readme)* remove --param-regex mention from CLI reference
- *(cli)* verify --param-regex is rejected as unknown argument
- *(builder)* cover multi-status response aggregation
- refactor!(error): mark Error enum as non_exhaustive
- regenerate demo.gif [skip ci]

## [0.3.0](https://github.com/Arkptz/mitm2openapi/compare/v0.2.6...v0.3.0) - 2026-04-22

### Added

- *(report)* track cap firings and parse errors in processing report
- *(cli)* add --report flag for structured processing summary
- *(tnetstring)* emit byte offset and error kind on parse halt

### Other

- *(readme)* document --report flag and parse halt diagnostics
- *(report)* verify report file schema and contents
- *(tnetstring)* verify parse halt diagnostics and no-resync on binary payload

## [0.2.6](https://github.com/Arkptz/mitm2openapi/compare/v0.2.5...v0.2.6) - 2026-04-22

### Fixed

- *(test)* gate Unix-specific path-failure test behind cfg(unix)
- *(output)* write YAML via tempfile and atomic rename

### Other

- *(output)* verify atomic write preserves target on failure
- *(deps)* move tempfile to runtime dependencies

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
