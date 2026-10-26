**English** | [Čeština](README.cs.md)

[![ci](https://github.com/meliodascz89/jkva/actions/workflows/ci.yml/badge.svg)](https://github.com/meliodascz89/jkva/actions/workflows/ci.yml)

# JKva

> A browser engine written from scratch in Rust. Apple Silicon first, megabytes last.

JKva is an early-stage browser engine: its own DOM, style system, layout engine, rasterizer, compositor and event loop — no inherited twenty-year-old C++ layer, no Chromium fork.

**Status: M2 — flexbox (subset), gaming platform started.** HTML parses through html5ever, CSS is parsed and cascaded, block, inline and flex layout run in `jkva-layout`, text is shaped and rasterized, and Metal presents the frame. `examples/hello.html`, `cards.html` and `flex.html` render in the screenshot corpus with goldens. Flex supports row/column, `justify-content`, `align-items`, `align-self`, `gap`, `flex-wrap`, `flex-grow`, `flex-shrink` and `flex-basis`. Gaming is a first-class 0.1 goal ([ADR-026](briefing/DECISIONS.md)): the permission-gated UDP transport is in `jkva-net` (default deny, per-application and per-origin grants), and the GPU performance probe is next.

![The first page rendered by JKva](docs/images/first-page.png)

*The first page rendered end to end by JKva: our DOM, our cascade, our block layout, our text shaping and Metal. `examples/hello.html`, 800×600.*

## Where we are (2026-10-25)

- **M0 complete** — repository, CI, EN+CS documentation, 24 ADRs, Metal walking skeleton, IPC contracts (`jkva-ipc`), and the harness: reftest diff, corpus runner, WPT report, WPT reftests, benchmarks.
- **M1 complete** — HTML → DOM → CSS cascade → block, inline and flex layout → shaped text → Metal → PNG; three golden pages; damage tracking seam; WPT reftest runner with published caveats.
- **M2 in progress** — flexbox: `row`/`column`, `justify-content`, `align-items`, `align-self`, `gap`, `wrap`, `grow`/`shrink`/`basis`. Next: `order`, `align-content`, grid. Gaming: ADR-026, permission-gated UDP, frame pacing, GPU capability report and the first frame-time baseline (CPU submit p95 0.15 ms, GPU execution p95 1.22 ms on a 4096-draw scene) and the engine frame after glyph/font/shaping caches (p50 about 0.51 ms) done.
- 57 test suites green, CI green, ~1.94 ms cascade on the 602-node fixture.

The checkable list, with evidence and known gaps, is [`docs/src/en/status.md`](docs/src/en/status.md) ([Czech](docs/src/cs/status.md)).

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

## Support the project

JKva has no paywall, no tokens, no paid features and no roadmap influence for sale. If you still want to fund the work, the only address is:

**Bitcoin (BIP84, native segwit):** `bc1qg9y4mwhc4c5ekexc4nfr6vxynfq0m8nq9vqqj2`

A donation is not a purchase: it does not buy features, priority or a say in the architecture. The project stays public under the MIT license, and every decision stays in [`briefing/DECISIONS.md`](briefing/DECISIONS.md).

## License

MIT. See [`LICENSE-MIT`](LICENSE-MIT).
