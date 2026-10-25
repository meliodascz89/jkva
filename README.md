**English** | [Čeština](README.cs.md)

# JKva

> A browser engine written from scratch in Rust. Apple Silicon first, megabytes last.

JKva is an early-stage browser engine: its own DOM, style system, layout engine, rasterizer, compositor and event loop — no inherited twenty-year-old C++ layer, no Chromium fork.

**Status: M0 — foundations.** The engine does not render a page yet; the first pixels are milestone M1. What works today is the repository, the CI, the documentation skeleton and a Metal probe that runs on Apple Silicon.

## What is in the repository

| Path | What it is |
|---|---|
| `crates/` | the engine, one crate per layer (`jkva-core`, `jkva-dom`, `jkva-style`, `jkva-layout`, `jkva-text`, `jkva-paint`, `jkva-gfx`, `jkva-js`, `jkva-api`, `jkva-engine`, `jkva-platform`, `jkva-shell`) |
| `briefing/` | the 99 decisions behind the architecture, the answers, and the ADRs derived from them |
| `research/` | verified background on the 2026 crate stack, benchmarks and prior art |
| `docs/` | documentation in English and Czech (mdBook) |
| `decisions/` | pointer to the decision record |

## Build

```bash
# requires Rust stable (edition 2024) and macOS on Apple Silicon
cargo build --workspace
cargo run -p jkva-platform --bin metal-probe   # prints the Metal device
cargo test --workspace
```

The full toolchain, targets and CI are described in [`docs/src/en/building.md`](docs/src/en/building.md).

## Where the decisions live

- [`briefing/DECISIONS.md`](briefing/DECISIONS.md) — 24 ADRs: scope, architecture, measurement, team.
- [`briefing/BRIEFING-0.1.cs.md`](briefing/BRIEFING-0.1.cs.md) — the 99 questions and the chosen answers.
- [`briefing/ANSWERS.md`](briefing/ANSWERS.md) — answers, including the deviations from the recommendations.

## License

MIT OR Apache-2.0, at your option. See [`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE).
