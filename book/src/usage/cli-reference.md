# CLI reference

<!-- toc -->

```admonish warning
This reference was last synced with `mitm2openapi --help` at version 0.5.1.
If you notice a flag missing from your local `--help` output, the tool may be ahead of these
docs. [Open an issue](https://github.com/Arkptz/mitm2openapi/issues/new) to prompt an update.
```

## `mitm2openapi discover`

Scan captured traffic and produce a templates file listing all observed endpoints.

```
mitm2openapi discover [OPTIONS] -i <INPUT> -o <OUTPUT> -p <PREFIX>
```

### Required arguments

| Option | Description |
|--------|-------------|
| `-i, --input <PATH>` | Input file (flow dump or HAR) |
| `-o, --output <PATH>` | Output YAML templates file |
| `-p, --prefix <URL>` | API prefix URL to filter requests |

### Optional arguments

| Option | Default | Description |
|--------|---------|-------------|
| `--format <FORMAT>` | `auto` | Input format: `auto`, `har`, `mitmproxy` |
| `--exclude-patterns <GLOBS>` | | Comma-separated globs; matching paths are dropped entirely |
| `--include-patterns <GLOBS>` | | Comma-separated globs; matching paths are auto-activated |
| `--max-input-size <BYTES>` | `2GiB` | Maximum input file size. Accepts `KiB`, `MiB`, `GiB` suffixes |
| `--allow-symlinks` | off | Allow symlinked input files |
| `--strict` | off | Treat warnings as errors (exit code 2) |
| `--report <PATH>` | | Write structured JSON processing report |

## `mitm2openapi generate`

Generate an OpenAPI 3.0 spec from captured traffic using a curated templates file.

```
mitm2openapi generate [OPTIONS] -i <INPUT> -t <TEMPLATES> -o <OUTPUT> -p <PREFIX>
```

### Required arguments

| Option | Description |
|--------|-------------|
| `-i, --input <PATH>` | Input file (flow dump or HAR) |
| `-t, --templates <PATH>` | Templates YAML file (from `discover`) |
| `-o, --output <PATH>` | Output OpenAPI YAML file |
| `-p, --prefix <URL>` | API prefix URL |

### Optional arguments

| Option | Default | Description |
|--------|---------|-------------|
| `--format <FORMAT>` | `auto` | Input format: `auto`, `har`, `mitmproxy` |
| `--openapi-title <TITLE>` | | Custom title for the spec |
| `--openapi-version <VER>` | `1.0.0` | Custom spec version |
| `--exclude-headers <LIST>` | | Comma-separated headers to exclude from spec |
| `--exclude-cookies <LIST>` | | Comma-separated cookies to exclude from spec |
| `--include-headers` | off | Include request headers in the spec |
| `--ignore-images` | off | Ignore image content types |
| `--suppress-params` | off | Suppress parameter suggestions |
| `--tags-overrides <JSON>` | | JSON string for tag overrides |
| `--max-input-size <BYTES>` | `2GiB` | Maximum input file size |
| `--max-payload-size <BYTES>` | `256MiB` | Maximum tnetstring payload size |
| `--max-depth <N>` | `256` | Maximum tnetstring nesting depth |
| `--max-body-size <BYTES>` | `64MiB` | Maximum request/response body size |
| `--allow-symlinks` | off | Allow symlinked input files |
| `--strict` | off | Treat warnings as errors (exit code 2) |
| `--report <PATH>` | | Write structured JSON processing report |

## Common flag details

### `--format`

By default, the input format is auto-detected from a combination of file extension and
content sniffing:
- `.flow` extension or content starting with a tnetstring length prefix → mitmproxy format
- `.har` extension or content starting with `{` → HAR format

Use `--format mitmproxy` or `--format har` to override auto-detection.

### `--prefix`

The prefix URL filters which requests are processed. Only requests whose URL starts with
the prefix are included. The prefix is stripped from paths in the generated spec.

Example: with `--prefix https://api.example.com`, a request to
`https://api.example.com/users/42` produces path `/users/42` in the spec.

### `--strict`

See [strict mode](./strict-mode.md) for details on exit codes and CI usage.

### `--report`

See [processing reports](./reports.md) for the JSON schema and CI integration examples.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Fatal error (I/O failure, missing arguments, invalid input) |
| 2 | Strict mode violation (warnings with `--strict` enabled) |

## Environment variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Controls log verbosity. Default: `warn`. Set to `info` or `debug` for more output. |

```bash
RUST_LOG=info mitm2openapi discover -i capture.flow -o templates.yaml -p "https://api.example.com"
```
