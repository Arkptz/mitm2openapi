# Filtering endpoints

<!-- toc -->

The `discover` command supports glob-based filters to automate endpoint curation.
This is useful for CI pipelines or large captures where manual editing is impractical.

## Glob syntax

Filters use git-style glob patterns (powered by the [`globset`](https://docs.rs/globset) crate):

| Pattern | Matches | Does not match |
|---------|---------|----------------|
| `*` | Single path segment | Segments with `/` |
| `**` | Any number of path segments | (matches everything) |
| `?` | Any single character | |
| `[abc]` | Character class | |
| `{a,b}` | Alternation | |

## `--exclude-patterns`

Paths matching any exclude glob are **dropped entirely** — they do not appear in the
templates file at all.

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --exclude-patterns '/static/**,/images/**,*.css,*.js,*.svg,*.png'
```

Multiple patterns are comma-separated. A path is excluded if it matches **any** pattern.

## `--include-patterns`

Paths matching any include glob are emitted **without the `ignore:` prefix** — they are
auto-activated for the `generate` step.

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --include-patterns '/api/**,/v2/**'
```

## Combining filters

When both are specified:

1. **Exclude runs first** — matching paths are dropped entirely
2. **Include runs second** — matching paths among the survivors are auto-activated
3. **Everything else** gets the `ignore:` prefix for manual review

```bash
mitm2openapi discover \
  -i capture.flow \
  -o templates.yaml \
  -p "https://api.example.com" \
  --exclude-patterns '/static/**,*.css,*.js' \
  --include-patterns '/api/**'
```

Result:
- `/static/bundle.js` — excluded (dropped)
- `/api/users` — included (auto-activated)
- `/dashboard` — neither matched (gets `ignore:` prefix)

## Examples

### API-only spec

```bash
--include-patterns '/api/**' \
--exclude-patterns '/api/internal/**,/api/debug/**'
```

### Strip static assets

```bash
--exclude-patterns '/static/**,/assets/**,*.css,*.js,*.svg,*.png,*.jpg,*.gif,*.ico,*.woff,*.woff2'
```

### Multiple API versions

```bash
--include-patterns '/v1/**,/v2/**,/v3/**'
```

## Pattern tips

- Patterns match against the **URL path only** (after the prefix is stripped), not the full URL
- Leading `/` is recommended for clarity but not required
- Patterns are case-sensitive
- Use `**` sparingly — it matches everything, including deeply nested paths
