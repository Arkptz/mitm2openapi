# Quick start

This walkthrough takes you from a traffic capture to a complete OpenAPI spec in under a minute.

## Prerequisites

- `mitm2openapi` installed ([see installation](./installation.md))
- A captured traffic file — either a mitmproxy `.flow` dump or a `.har` export from browser DevTools

If you do not have a capture yet, see [capturing traffic](./capturing.md) for setup instructions.

## Step 1: Discover endpoints

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com"
```

This scans every request in `capture.flow` that matches the prefix `https://api.example.com`
and writes a templates file listing all observed URL paths.

## Step 2: Curate the templates

Open `templates.yaml`. Each path is prefixed with `ignore:` by default:

```yaml
- ignore:/api/users
- ignore:/api/users/{id}
- ignore:/api/products
- ignore:/static/bundle.js
```

Remove the `ignore:` prefix from paths you want in the final spec:

```yaml
- /api/users
- /api/users/{id}
- /api/products
- ignore:/static/bundle.js
```

Paths still prefixed with `ignore:` are excluded from the generated spec.

## Step 3: Generate the OpenAPI spec

```bash
mitm2openapi generate \
  -i capture.flow \
  -t templates.yaml \
  -o openapi.yaml \
  -p "https://api.example.com"
```

The resulting `openapi.yaml` contains a valid OpenAPI 3.0 spec with paths, methods,
parameters, request bodies, and response schemas inferred from the captured traffic.

## Skip the manual edit

If you already know which paths matter, use glob filters to automate curation:

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --exclude-patterns '/static/**,/images/**,*.css,*.js,*.svg' \
  --include-patterns '/api/**,/v2/**'

mitm2openapi generate \
  -i capture.flow \
  -t templates.yaml \
  -o openapi.yaml \
  -p "https://api.example.com"
```

Paths matching `--include-patterns` are auto-activated (no `ignore:` prefix). Paths matching
`--exclude-patterns` are dropped entirely. Everything else still gets `ignore:` for manual
review.

See [filtering endpoints](../usage/filtering.md) for the full glob syntax reference.

## HAR files

The same workflow works with HAR files — just point `-i` at a `.har` file. The format is
auto-detected:

```bash
mitm2openapi discover \
  -i capture.har \
  -o templates.yaml \
  -p "https://api.example.com"
```

See [HAR files](../formats/har.md) for details on exporting HARs from browser DevTools.
