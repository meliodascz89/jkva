# Building

## Requirements

- macOS on Apple Silicon (M1 or newer)
- Rust stable 1.85+ with `rustfmt` and `clippy`
- Xcode command line tools (for the Metal SDK headers)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal -c rustfmt -c clippy
```

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --open
```

## Metal probe

The first thing M0 proves is that the Metal binding layer works on this machine:

```bash
cargo run -p jkva-platform --bin metal-probe
# Metal device: Apple M4
# Apple9 family: yes
```

If it prints `Metal is not available`, the engine can still build and run its CPU tests, but the GPU path is unavailable.

## CI

- macOS job: format, clippy, tests, docs on Apple Silicon.
- Linux job: `cargo check` for the portable crates (`jkva-platform` and `jkva-shell` are macOS-only).

The self-hosted M4 runner is used for benchmarks, not for every pull request.
