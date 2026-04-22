# PoC fixtures

Binary proof-of-concept fixtures used by hardening regression tests.

## Naming scheme

```
<finding_id>_<description>.bin
```

Examples:

- `P1.1_payload_1tb.bin` — oversized tnetstring payload triggering the body-size cap
- `P1.2_nested_50k.bin` — deeply nested tnetstring triggering the depth limit

Each fixture is the minimal reproduction case for a specific hardening finding.
Later hardening PRs drop binaries here as they land.
