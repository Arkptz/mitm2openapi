use std::path::Path;

use anyhow::{bail, Context, Result};
use clap::Parser;
use tracing::{debug, info, warn};

use mitm2openapi::builder::{self, OpenApiBuilder};
use mitm2openapi::cli::{Cli, Command, InputFormat};
use mitm2openapi::har_reader;
use mitm2openapi::mitmproxy_reader;
use mitm2openapi::output;
use mitm2openapi::report::ProcessingReport;
use mitm2openapi::types::{CapturedRequest, Config};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Discover(args) => {
            info!(input = %args.input.display(), output = %args.output.display(), "Starting discovery");

            let mut report = ProcessingReport::new();
            report.input.path = args.input.display().to_string();
            report.input.format = format!("{:?}", args.format);
            if let Ok(meta) = std::fs::metadata(&args.input) {
                report.input.size_bytes = meta.len();
            }

            let req_iter = stream_input(
                &args.input,
                &args.format,
                args.max_input_size,
                args.allow_symlinks,
            )?;

            let templates = builder::discover_paths_streaming(
                req_iter,
                &args.prefix,
                None,
                &args.exclude_patterns,
                &args.include_patterns,
            );

            let merged = if args.output.exists() {
                let existing = load_templates(&args.output)
                    .context("failed to load existing templates file")?;
                debug!(
                    existing = existing.len(),
                    new = templates.len(),
                    "Merging templates"
                );
                merge_templates(&existing, &templates)
            } else {
                templates
            };

            report.result.paths_in_spec = merged.len() as u64;

            let yaml = output::templates_to_yaml(&merged)?;
            output::write_yaml(&yaml, &args.output)?;

            if let Some(report_path) = &args.report {
                report.write_to_path(report_path).with_context(|| {
                    format!("failed to write report to {}", report_path.display())
                })?;
                info!(path = %report_path.display(), "Processing report written");
            }

            info!(
                count = merged.len(),
                output = %args.output.display(),
                "Discovery complete"
            );
            eprintln!(
                "Discovered {} path template(s), written to {}",
                merged.len(),
                args.output.display()
            );
        }
        Command::Generate(args) => {
            info!(input = %args.input.display(), output = %args.output.display(), "Starting generation");

            let mut report = ProcessingReport::new();
            report.input.path = args.input.display().to_string();
            report.input.format = format!("{:?}", args.format);
            if let Ok(meta) = std::fs::metadata(&args.input) {
                report.input.size_bytes = meta.len();
            }

            let req_iter = stream_input(
                &args.input,
                &args.format,
                args.max_input_size,
                args.allow_symlinks,
            )?;

            let all_templates = load_templates(&args.templates).with_context(|| {
                format!("failed to load templates from {}", args.templates.display())
            })?;
            let active_templates: Vec<String> = all_templates
                .into_iter()
                .filter(|t| !t.starts_with("ignore:"))
                .collect();

            if active_templates.is_empty() {
                bail!(
                    "No active templates found in {}. Remove the 'ignore:' prefix from paths you want to include.",
                    args.templates.display()
                );
            }

            info!(count = active_templates.len(), "Using active templates");

            let config = Config {
                prefix: args.prefix.clone(),
                param_regex: args.param_regex.clone(),
                openapi_title: args.openapi_title.clone(),
                openapi_version: args.openapi_version.clone(),
                exclude_headers: parse_comma_list(&args.exclude_headers),
                exclude_cookies: parse_comma_list(&args.exclude_cookies),
                include_headers: args.include_headers,
                ignore_images: args.ignore_images,
                suppress_params: args.suppress_params,
                tags_overrides: args.tags_overrides.clone(),
            };

            let mut builder = OpenApiBuilder::new(&args.prefix, &config, active_templates);
            let mut count = 0usize;
            for req_result in req_iter {
                report.result.flows_read += 1;
                match req_result {
                    Ok(req) => {
                        builder.add_request(req.as_ref());
                        report.result.flows_emitted += 1;
                        count += 1;
                    }
                    Err(e) => {
                        let error_key = format!("{}", e);
                        *report.events.parse_error.entry(error_key).or_insert(0) += 1;
                        warn!(error = %e, "Skipping request due to error");
                    }
                }
            }
            info!(count, path = %args.input.display(), "Processed requests");
            let spec = builder.build();

            let path_count = spec.paths.paths.len();
            report.result.paths_in_spec = path_count as u64;

            let yaml = output::spec_to_yaml(&spec)?;
            output::write_yaml(&yaml, &args.output)?;

            if let Some(report_path) = &args.report {
                report.write_to_path(report_path).with_context(|| {
                    format!("failed to write report to {}", report_path.display())
                })?;
                info!(path = %report_path.display(), "Processing report written");
            }

            info!(
                paths = path_count,
                output = %args.output.display(),
                "Generation complete"
            );
            eprintln!(
                "Generated OpenAPI spec with {} path(s), written to {}",
                path_count,
                args.output.display()
            );
        }
    }

    Ok(())
}

/// Heuristic scoring for format auto-detection.
/// Returns (mitmproxy_score, har_score) — higher score means more confidence.
fn detect_format_score(path: &Path) -> (u8, u8) {
    let mut mitmproxy_score: u8 = 0;
    let mut har_score: u8 = 0;

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_lowercase().as_str() {
            "flow" => mitmproxy_score += 3,
            "har" => har_score += 3,
            _ => {}
        }
    }

    if mitmproxy_reader::mitmproxy_heuristic(path) {
        mitmproxy_score += 2;
    }
    if har_reader::har_heuristic(path) {
        har_score += 2;
    }

    (mitmproxy_score, har_score)
}

type RequestIter = Box<dyn Iterator<Item = mitm2openapi::error::Result<Box<dyn CapturedRequest>>>>;

fn stream_input(
    path: &Path,
    format: &InputFormat,
    max_input_size: u64,
    allow_symlinks: bool,
) -> Result<RequestIter> {
    if !path.is_dir() {
        mitm2openapi::validate_input_path(path, max_input_size, allow_symlinks)
            .context("input file validation failed")?;
    }
    match format {
        InputFormat::Mitmproxy => {
            debug!(path = %path.display(), "Streaming as mitmproxy format");
            if path.is_dir() {
                mitmproxy_reader::stream_mitmproxy_dir(path)
                    .context("failed to stream mitmproxy directory")
            } else {
                let iter = mitmproxy_reader::stream_mitmproxy_file(path)
                    .context("failed to stream mitmproxy file")?;
                Ok(Box::new(iter))
            }
        }
        InputFormat::Har => {
            debug!(path = %path.display(), "Streaming as HAR format");
            let iter = har_reader::stream_har_file(path).context("failed to stream HAR file")?;
            Ok(Box::new(iter))
        }
        InputFormat::Auto => {
            if path.is_dir() {
                debug!(path = %path.display(), "Auto-detecting format for directory");
                let mitmproxy_result = mitmproxy_reader::stream_mitmproxy_dir(path);
                let har_result = har_reader::stream_har_file(path);

                match (mitmproxy_result, har_result) {
                    (Ok(m_iter), Ok(h_iter)) => {
                        let combined: RequestIter = Box::new(m_iter.chain(h_iter));
                        Ok(combined)
                    }
                    (Ok(m_iter), Err(_)) => Ok(m_iter),
                    (Err(_), Ok(h_iter)) => Ok(h_iter),
                    (Err(e1), Err(_e2)) => {
                        Err(e1).context("failed to read directory as mitmproxy or HAR")
                    }
                }
            } else {
                let (ms, hs) = detect_format_score(path);
                debug!(
                    path = %path.display(),
                    mitmproxy_score = ms,
                    har_score = hs,
                    "Format auto-detection scores"
                );

                if ms > hs {
                    info!(path = %path.display(), "Auto-detected as mitmproxy format");
                    let iter = mitmproxy_reader::stream_mitmproxy_file(path)
                        .context("detected as mitmproxy format but failed to parse")?;
                    Ok(Box::new(iter))
                } else if hs > ms {
                    info!(path = %path.display(), "Auto-detected as HAR format");
                    let iter = har_reader::stream_har_file(path)
                        .context("detected as HAR format but failed to stream")?;
                    Ok(Box::new(iter))
                } else if ms > 0 {
                    warn!(path = %path.display(), "Ambiguous format detection, trying mitmproxy first");
                    match mitmproxy_reader::stream_mitmproxy_file(path) {
                        Ok(iter) => Ok(Box::new(iter)),
                        Err(_) => {
                            let iter = har_reader::stream_har_file(path)
                                .context("failed to parse as either mitmproxy or HAR")?;
                            Ok(Box::new(iter))
                        }
                    }
                } else {
                    bail!(
                        "Cannot auto-detect format for '{}'. Use --format to specify.",
                        path.display()
                    );
                }
            }
        }
    }
}

fn load_templates(path: &Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read templates file: {}", path.display()))?;
    let yaml: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&content).context("failed to parse templates YAML")?;

    let templates = yaml
        .get("x-path-templates")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(templates)
}

fn merge_templates(existing: &[String], new: &[String]) -> Vec<String> {
    use std::collections::BTreeSet;

    let existing_paths: BTreeSet<String> = existing
        .iter()
        .map(|t| t.strip_prefix("ignore:").unwrap_or(t).to_string())
        .collect();

    let mut merged: Vec<String> = existing.to_vec();

    for t in new {
        let bare = t.strip_prefix("ignore:").unwrap_or(t);
        if !existing_paths.contains(bare) {
            merged.push(t.clone());
        }
    }

    merged
}

fn parse_comma_list(opt: &Option<String>) -> Vec<String> {
    opt.as_deref()
        .map(|s| {
            s.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}
