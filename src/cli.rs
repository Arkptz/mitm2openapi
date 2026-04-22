use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Convert mitmproxy/HAR captures to OpenAPI 3.0 specifications
#[derive(Parser, Debug)]
#[command(name = "mitm2openapi", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Discover API endpoints from captured traffic and produce a templates file
    Discover(DiscoverArgs),
    /// Generate an OpenAPI specification from captured traffic using a templates file
    Generate(GenerateArgs),
}

/// Input format for traffic captures
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum InputFormat {
    /// Auto-detect format from file extension/content
    #[default]
    Auto,
    /// HAR (HTTP Archive) format
    Har,
    /// mitmproxy flow dump format
    Mitmproxy,
}

fn parse_byte_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix("GiB") {
        (n.trim(), 1024 * 1024 * 1024u64)
    } else if let Some(n) = s.strip_suffix("MiB") {
        (n.trim(), 1024 * 1024u64)
    } else if let Some(n) = s.strip_suffix("KiB") {
        (n.trim(), 1024u64)
    } else {
        (s, 1u64)
    };
    num_str
        .parse::<u64>()
        .map(|n| n * multiplier)
        .map_err(|e| format!("invalid size: {e}"))
}

#[derive(Parser, Debug)]
pub struct DiscoverArgs {
    /// Input file or directory path
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output YAML file path for discovered templates
    #[arg(short, long)]
    pub output: PathBuf,

    /// API prefix URL (e.g., "https://api.example.com")
    #[arg(short, long)]
    pub prefix: String,

    /// Input format override
    #[arg(long, value_enum, default_value_t = InputFormat::Auto)]
    pub format: InputFormat,

    /// Comma-separated glob patterns for paths to drop from output entirely
    /// (e.g. "/static/**,/images/**,*.css,*.js").
    /// Use `*` for a single path segment, `**` for any number of segments.
    #[arg(long, value_delimiter = ',')]
    pub exclude_patterns: Vec<String>,

    /// Comma-separated glob patterns for paths to emit WITHOUT the `ignore:`
    /// prefix (i.e. auto-activate for generate). Everything else still gets
    /// `ignore:` so you can review it. Saves a manual sed step.
    #[arg(long, value_delimiter = ',')]
    pub include_patterns: Vec<String>,

    #[arg(long, value_parser = parse_byte_size, default_value = "2GiB")]
    pub max_input_size: u64,

    #[arg(long, default_value_t = false)]
    pub allow_symlinks: bool,

    /// Treat warnings as errors. Exits non-zero if any cap fires,
    /// flow is rejected, or parse diagnostic is emitted.
    #[arg(long, default_value_t = false)]
    pub strict: bool,

    /// Write a structured JSON processing report to the given path
    #[arg(long)]
    pub report: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct GenerateArgs {
    /// Input file or directory path
    #[arg(short, long)]
    pub input: PathBuf,

    /// Templates YAML file path (from discover output)
    #[arg(short, long)]
    pub templates: PathBuf,

    /// Output OpenAPI YAML file path
    #[arg(short, long)]
    pub output: PathBuf,

    /// API prefix URL
    #[arg(short, long)]
    pub prefix: String,

    /// Input format override
    #[arg(long, value_enum, default_value_t = InputFormat::Auto)]
    pub format: InputFormat,

    /// Custom title for the OpenAPI spec
    #[arg(long)]
    pub openapi_title: Option<String>,

    /// Custom version for the OpenAPI spec
    #[arg(long, default_value = "1.0.0")]
    pub openapi_version: String,

    /// Comma-separated headers to exclude
    #[arg(long)]
    pub exclude_headers: Option<String>,

    /// Comma-separated cookies to exclude
    #[arg(long)]
    pub exclude_cookies: Option<String>,

    /// Include headers in the generated spec
    #[arg(long)]
    pub include_headers: bool,

    /// Ignore image content types
    #[arg(long)]
    pub ignore_images: bool,

    /// Suppress parameter suggestions
    #[arg(long)]
    pub suppress_params: bool,

    /// JSON string for tag overrides
    #[arg(long)]
    pub tags_overrides: Option<String>,

    #[arg(long, value_parser = parse_byte_size, default_value = "2GiB")]
    pub max_input_size: u64,

    #[arg(long, value_parser = parse_byte_size, default_value = "256MiB")]
    pub max_payload_size: u64,

    #[arg(long, default_value_t = 256)]
    pub max_depth: usize,

    #[arg(long, value_parser = parse_byte_size, default_value = "64MiB")]
    pub max_body_size: u64,

    #[arg(long, default_value_t = false)]
    pub allow_symlinks: bool,

    /// Treat warnings as errors. Exits non-zero if any cap fires,
    /// flow is rejected, or parse diagnostic is emitted.
    #[arg(long, default_value_t = false)]
    pub strict: bool,

    /// Write a structured JSON processing report to the given path
    #[arg(long)]
    pub report: Option<PathBuf>,
}
