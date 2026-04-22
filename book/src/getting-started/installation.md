# Installation

## From binary releases

Download a pre-built binary for your platform from
[GitHub Releases](https://github.com/Arkptz/mitm2openapi/releases).

Binaries are available for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and
Windows (x86_64).

```bash
# Example: Linux x86_64
curl -L https://github.com/Arkptz/mitm2openapi/releases/latest/download/mitm2openapi-x86_64-unknown-linux-gnu.tar.gz \
  | tar xz
sudo mv mitm2openapi /usr/local/bin/
```

## From source (via Cargo)

If you have a Rust toolchain installed:

```bash
cargo install --git https://github.com/Arkptz/mitm2openapi
```

Or from [crates.io](https://crates.io/crates/mitm2openapi):

```bash
cargo install mitm2openapi
```

## Verify installation

```bash
mitm2openapi --version
```

## Shell completions

`mitm2openapi` uses [clap](https://docs.rs/clap) for argument parsing. Shell completions
are not yet bundled, but you can generate them for most shells via `clap_complete` if building
from source.
