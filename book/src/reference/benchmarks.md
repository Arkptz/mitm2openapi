# Performance & benchmarks

Automated CI benchmarks run weekly against the Python original
([mitmproxy2swagger](https://github.com/alufers/mitmproxy2swagger)). You can trigger a
fresh run via
[Actions → Benchmark](https://github.com/Arkptz/mitm2openapi/actions/workflows/bench.yml).

The raw data lives in [`docs/benchmarks.md`](https://github.com/Arkptz/mitm2openapi/blob/main/docs/benchmarks.md)
and is auto-updated by the benchmark workflow.

## Latest results

_Run: 2026-04-22, commit `22ef2faa`, runner: Linux 6.17.0-1011-azure_

Fixture: 89 MB, 40k requests across 8 endpoint shapes.

### Timing

| Command | Mean | Min | Max | Relative |
|:---|---:|---:|---:|---:|
| Python mitmproxy2swagger | 44.757 ± 0.219 s | 44.384 s | 44.965 s | 16.80x |
| Rust mitm2openapi | 2.663 ± 0.039 s | 2.618 s | 2.712 s | 1.00x |

### Peak RSS

| Tool | RSS |
|------|----:|
| Python mitmproxy2swagger | 46 MB |
| Rust mitm2openapi | 6 MB |

## Methodology

Both tools process the same 89 MB synthetic capture containing 40,000 requests across
8 endpoint shapes. Timing is measured with [hyperfine](https://github.com/sharkdp/hyperfine)
(5 runs, 1 warmup). Peak RSS is measured via `/usr/bin/time -v`.

The fixture is generated deterministically by the benchmark workflow and stored as a GitHub
release asset for reproducibility.

## Reproducing locally

```bash
# Download the benchmark fixture
gh release download bench-fixtures-v1 \
  --repo Arkptz/mitm2openapi \
  --pattern 'bench-fixture-*.flow'

# Run the Rust tool
hyperfine --warmup 1 --runs 5 \
  'mitm2openapi discover -i bench-fixture.flow -o /dev/null \
    -p "http://petstore:8080"'

# Run the Python tool (requires mitmproxy2swagger installed)
hyperfine --warmup 1 --runs 5 \
  'mitmproxy2swagger -i bench-fixture.flow -o /dev/null \
    -p "http://petstore:8080"'
```
