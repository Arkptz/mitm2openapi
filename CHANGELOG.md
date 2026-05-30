# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.2](https://github.com/Arkptz/mitm2openapi/compare/v0.7.1...v0.7.2) - 2026-05-30

### Other

- release v0.7.1

## [0.7.1](https://github.com/Arkptz/mitm2openapi/compare/v0.7.0...v0.7.1) - 2026-05-30

### Other

- update Cargo.lock dependencies

## [0.7.0](https://github.com/Arkptz/mitm2openapi/compare/v0.6.0...v0.7.0) - 2026-05-28

### Added

- *(generate)* integrate envelope detection into build()
- *(generate)* integrate operationId strategy into builder
- *(cli)* add envelope detection CLI flags
- *(generate)* integrate tag rules strategy into builder
- add envelope module for discriminator-based response splitting
- *(cli)* add operationId and tag strategy CLI flags
- add operation_id module with camelCase derivation and collision resolution
- add tag_rules module with regex-based tag matching

### Fixed

- *(cli)* show correct value name and help text for --redact-patterns
- *(cli)* stop splitting --redact-patterns on comma, fail invalid regex under --strict
- *(envelope)* pin discriminator field to boolean `enum: [false]` in ApiError
- *(envelope)* merge all error bodies in infer_api_error, not just first
- *(test)* normalize CRLF in snapshot comparison, add .gitattributes eol=lf
- *(output)* sort maps for deterministic YAML output

### Other

- *(changelog)* add bugfix entries from MEXC integration testing
- document operationId, tags, and envelope features
- *(integration)* add end-to-end test for operationId, tags, and envelope
- add snapshot test pinning current generate output

### Added

- *(generate)* add `--operation-id-strategy {none,path,template}` flag for stable operationId generation
- *(generate)* add `--operation-id-overrides <path>` for per-operation overrides via YAML
- *(generate)* add `--operation-id-template <string>` for custom operationId templates
- *(generate)* add `--tag-strategy {legacy,none,path-segment,rules}` flag
- *(generate)* add `--tag-rules <path>` for regex-based tag assignment from YAML
- *(generate)* add `--tag-segment-index <N>` to use a specific path segment as tag
- *(generate)* add `--envelope-discriminator <field>` for discriminator-based response splitting
- *(generate)* add `--envelope-error-shape <path>` for hand-supplied ApiError schema
- *(generate)* add `--envelope-success-component-suffix <string>` (default `Success`)
- *(output)* sort paths and component schemas alphabetically for deterministic YAML output
- *(generate)* add `--enrichments <PATH>` (`-e`) flag for applying operationId-keyed YAML overlays with human-written metadata (summaries, descriptions, tags, `x-` extensions, response descriptions, component schema descriptions)

### Fixed

- *(envelope)* `infer_api_error` now merges all error bodies, not just the first — an outlier `msg: 0` no longer overrides thousands of `msg: "string"` samples
- *(envelope)* inferred `ApiError` schema now includes the discriminator field pinned with `enum: [false]`
- *(cli)* `--redact-patterns` no longer splits on `,` — regex quantifiers like `{32,}` now work correctly (pass multiple patterns via repeated flags)
- *(cli)* invalid `--redact-patterns` regex now hard-fails under `--strict` instead of silently skipping redaction

> Found and fixed by integration testing against a 3.1 GB MEXC capture in mexc-reversed-sdk.

## [0.6.0](https://github.com/Arkptz/mitm2openapi/compare/v0.5.2...v0.6.0) - 2026-05-27

### Added

- *(generate)* integrate redaction with collected examples
- *(generate)* collect request body examples
- *(cli)* add --max-examples flag with default cap of 5
- *(cli)* re-introduce --param-regex with functional implementation
- *(discover)* add cross-request variability detection for path params
- *(generate)* merge request body schemas across multiple captures
- add redaction module for example sanitization
- *(cli)* add --skip-options flag to filter OPTIONS requests
- *(discover)* extend param detection with UPPER_CASE, hex, base58 heuristics

### Fixed

- *(changelog)* remove duplicate Unreleased section
- *(mitmproxy)* wire --max-payload-size to tnetstring parser, fix clippy
- *(tests)* replace single-arm match with if let
- resolve clippy warnings and example name leaking sensitive values
- *(mitmproxy)* decompress response bodies based on content-encoding header

### Other

- *(integration)* add full pipeline test with all new features
- update cli-reference, pipeline, introduction, README for new features
- *(builder)* add examples accumulator infrastructure

### Added

- *(cli)* add `--skip-options` flag to filter OPTIONS requests (discover + generate)
- *(cli)* add `--param-regex` flag for custom path parameter detection (discover)
- *(cli)* add `--max-examples` flag to cap examples per endpoint (generate, default 5)
- *(cli)* add `--redact-patterns` flag for regex-based value redaction (generate)
- *(cli)* add `--redact-fields` flag for field-name-based redaction (generate)
- *(discover)* enhanced path parameterization: UPPER_CASE slugs, hex strings, base58, cross-request variability
- *(generate)* response/request examples stored as named OpenAPI examples
- *(generate)* request body schema merging via oneOf for multiple captures

## [0.5.2](https://github.com/Arkptz/mitm2openapi/compare/v0.5.1...v0.5.2) - 2026-04-24

### Fixed

- *(har)* apply header size caps consistent with mitmproxy reader
- *(reader)* reject symlinked directory inputs and entries

### Other

- *(security)* cover symlink directory and entry rejection
- *(readme)* trim content migrated to book, add docs badge
- *(book)* add mdBook scaffold with book.toml and all chapter content
- adjust CHANGELOG/CONTRIBUTING headings for mdBook inclusion
- regenerate demo.gif [skip ci]

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
