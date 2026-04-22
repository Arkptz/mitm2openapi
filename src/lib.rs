//! # mitm2openapi
//!
//! Convert mitmproxy flow dumps and HAR files to OpenAPI 3.0 specifications.
//!
//! This crate provides both a CLI binary (`mitm2openapi`) and a library for
//! programmatic use. It supports mitmproxy flow dumps (v19–v21) and HAR 1.2 files.
//!
//! ## Workflow
//!
//! 1. `discover` scans captured traffic and emits a curatable templates file.
//! 2. The user curates the templates (or uses `--include-patterns` / `--exclude-patterns`).
//! 3. `generate` produces the final OpenAPI 3.0 YAML spec.
//!
//! See the [CLI documentation][cli] and the [project README][readme] for examples.
//!
//! [cli]: crate::cli
//! [readme]: https://github.com/Arkptz/mitm2openapi#readme

pub mod builder;
pub mod cli;
pub mod error;
pub mod har_reader;
pub mod mitmproxy_reader;
pub mod output;
pub mod params;
pub mod path_matching;
pub mod schema;
pub mod tnetstring;
pub mod types;

/// Maximum size of a single input file (2 GiB).
pub const MAX_INPUT_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// Maximum size of a single TNetString payload (256 MiB).
pub const MAX_PAYLOAD_SIZE: usize = 256 * 1024 * 1024;

/// Maximum recursion depth for TNetString parsing.
pub const MAX_DEPTH: usize = 256;

/// Maximum recursion depth for JSON-to-schema conversion.
pub const MAX_SCHEMA_DEPTH: usize = 64;

/// Maximum body size for response/request bodies (64 MiB).
pub const MAX_BODY_SIZE: usize = 64 * 1024 * 1024;
