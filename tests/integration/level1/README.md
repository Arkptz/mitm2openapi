# Level 1 Integration Tests

Level 1 tests verify that `mitm2openapi` produces a valid OpenAPI spec from a
live Petstore service. The pipeline spins up Petstore via Docker Compose, seeds
traffic through mitmproxy, runs `discover` → `generate`, and then compares the
output against a golden baseline using [oasdiff](https://github.com/Tufin/oasdiff).

## Diff Modes

Level 1 uses two diff modes against the golden file `tests/golden/petstore-v3.yaml`:

| Mode | Flag | oasdiff option | Gate behaviour |
|------|------|----------------|----------------|
| **Naive** | _(default)_ | `--fail-on BREAKING` | Required — blocks merge on breaking changes |
| **Strict** | `--strict` | `--fail-on WARN` | Informational — visible in CI but does not block merge |

**Naive** catches structural regressions (removed paths, changed types, deleted
parameters). **Strict** additionally flags cosmetic or minor drifts (description
changes, example differences, new optional fields). Strict failures are signals,
not blockers.

## Running Locally

Prerequisites:

- Docker (for Petstore + mitmproxy containers)
- `oasdiff` binary on `$PATH`
- Built `mitm2openapi` binary at `target/release/mitm2openapi`

```bash
# Naive diff (required gate)
./run.sh

# Strict diff (informational gate)
./run.sh --strict
```

Both commands start Docker Compose, seed traffic, generate the spec, and run the
appropriate diff wrapper (`diff-naive.sh` or `diff-strict.sh`). Containers are
torn down automatically on exit.

## CI / Branch Protection Setup

In the GitHub Actions workflow, naive and strict run as separate jobs:

```yaml
jobs:
  level1-naive:    # runs diff-naive.sh
  level1-strict:   # runs diff-strict.sh
```

Configure branch protection in **GitHub → Settings → Branches → Branch
protection rules**:

1. Add `level1-naive` to **required status checks**. PRs cannot merge when this
   job fails.
2. Leave `level1-strict` **not required**. It still runs and its result is
   visible on the PR, but a failure does not block merge.

Do **not** use `continue-on-error: true` on the strict job. The job should
report red/green honestly; branch protection settings control whether red blocks
the merge.

## oasdiff Exceptions / Allow-List

When a known diff is acceptable (e.g. an intentional description change), add an
oasdiff configuration file rather than weakening the diff mode:

```bash
# Create or edit .oasdiff.yaml at repo root
oasdiff diff baseline.yaml generated.yaml \
  --fail-on WARN \
  --exclude-elements "description"
```

Alternatively, use `--match-path` / `--filter-extension` flags to scope checks
to specific paths. See the
[oasdiff docs](https://github.com/Tufin/oasdiff#configuration) for the full
list of filtering options.

## Escalation Policy

If a pattern causes strict-only failures across **3 or more PRs**:

1. Investigate whether the diff reflects a real regression or a gap in the
   golden file.
2. If it is a real regression, fix the generator.
3. If the golden file is stale, update it and reset the counter.
4. If the pattern is a genuine quality concern, escalate the check to required
   by adding `level1-strict` to the branch protection required checks.

## Files

| File | Purpose |
|------|---------|
| `run.sh` | Orchestrator — sets up Docker, generates spec, delegates to diff script |
| `diff-naive.sh` | Runs `oasdiff diff --fail-on BREAKING` |
| `diff-strict.sh` | Runs `oasdiff diff --fail-on WARN` |
| `docker-compose.yml` | Petstore + mitmproxy service definitions |
| `fixtures/` | Seed data and captured flow files |
