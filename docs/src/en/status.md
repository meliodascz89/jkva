## Session state (2026-10-25, end of day)

- Last code commit: `f3997d3` (`align-self`), CI green; this documentation update follows it.
- M0 and M1 are closed. M2 is in progress.
- Working tree is clean. The partial `flex-wrap`/`grow` work was stopped and reverted instead of being left half-finished, so the next developer starts from a green tree.
- Next step: flex wrap plus `flex-grow`/`flex-shrink`/`flex-basis` (four cascade properties and a line-breaking pass in `layout_flex_content`), then grid and tables.

# Milestone status

What is done, what is verified, and what is deliberately not done yet. Every line points at code or a document you can inspect.

## M0 — foundations

| Item | Status | Evidence |
|---|---|---|
| Repository, license, community files | done | `LICENSE-*`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md` |
| CI: format, clippy, tests, rustdoc, docs | done | `.github/workflows/ci.yml`, green on every push |
| Documentation skeleton EN + CS | done | `docs/`, this book |
| Architecture decisions | done | [`briefing/DECISIONS.md`](https://github.com/meliodascz89/jkva/blob/main/briefing/DECISIONS.md) |
| Walking skeleton: Metal device and triangle | done | `crates/jkva-gfx`, `gfx-clear`, `gfx-triangle` |
| IPC contracts | done | `crates/jkva-ipc`: versioned, length-prefixed frames; version, size and truncation checks |
| Harness: pixel diff, corpus runner, WPT report, WPT reftests, benchmarks | done | `tools/reftest-diff`, `tools/reftest-runner`, `tools/wpt-report`, `tools/wpt-reftest-runner`, `tools/bench-runner` |
| Public repository from the first commit | done | <https://github.com/meliodascz89/jkva> |

## M1 — first rendered page

| Item | Status | Evidence |
|---|---|---|
| HTML → DOM | done | `jkva-dom`, html5ever 0.40; 38 tests |
| CSS parse, selector matching, cascade, UA sheet | done | `jkva-style`; 157 tests |
| Block and inline layout | done | `jkva-layout`; 73 tests including flex; inline boxes, percentages, auto centring, wrapping |
| Text shaping and glyph rasterization | done | `jkva-text`, fontdb + harfrust + swash; 21 tests |
| Raster + compositing path | done | `jkva-gfx` scene renderer, alpha blending |
| Screenshot harness with golden pages | done | `examples/hello.html`, `examples/cards.html`, `tools/reftest-runner` |
| WPT reftest harness | done | `tools/wpt-reftest-runner`; first run 10 pass / 7 fail / 7 unsupported of 25 |
| Incremental seam | done | `jkva-shell::Frame` returns a `DamageRegion`; layout is still a full pass |
| Documentation and ADRs current | done | this page, `docs/`, `CHANGELOG.md` |

## M2 — flex (subset, in progress)

| Item | Status | Evidence |
|---|---|---|
| Flex formatting context | done | `layout_flex_content`; row/column, 5 justify modes, 4 align modes, gap |
| Flex reachable from the engine | done | the shell maps `display: flex` to `Display::Flex` |
| `align-self` per item | done | falls back to `align-items`; tested |
| Corpus page with a golden | done | `examples/flex.html`, `examples/expected/flex.png` |
| Wrap, `flex-grow`/`shrink`/`basis` | done | row wrapping, grow/shrink distribution, basis, percentage basis |
| `order` and `align-content` | done | stable order sort; six align-content modes for multi-line rows |
| Column wrap, `align-content` on columns, intrinsic min/max | not yet | next M2 increment |
| Grid, tables, floats, images, scroll, hit testing, layers | not yet | M2 |

## Gaming platform (ADR-026)

| Item | Status | Evidence |
|---|---|---|
| Gaming treated as a first-class 0.1 goal | decided | ADR-026 in `briefing/DECISIONS.md` |
| Permission-gated UDP (Tokio) | done | `jkva-net::udp`: default deny, per-origin and per-application grants, revoke, datagram size and byte budget |
| Permission gate | done | `jkva-api::permissions`: once/session grants, per origin+app, denial overrides, insecure origins rejected |
| Gamepad input over IPC v2 | done | `InputEvent::Gamepad` with buttons and axes |
| GPU command ABI | done | `jkva-gpu`: typed commands, device limits, balanced passes, versioned frames, 7 tests |
| Fixed-timestep frame pacing | done | `jkva-core::frame::FrameClock`: capped steps, interpolation alpha, rate change |
| GPU capability probe | done | `jkva-gfx::capabilities()`: Apple M4, 13.6 GiB max buffer, 18.2 GiB working set, timestamp counters |
| Frame-time benchmark | done | `tools/gfx-bench`: 4096 quads, 1080p, CPU submit p95 0.153 ms, GPU execution p95 1.221 ms |
| Engine frame benchmark | done | `tools/engine-bench`: cards 800x600 paint+submit p50 0.51 / p95 2.46 ms after glyph, font and shaping caches |
| Animated game frame + damage | done | 2000 sprites: incremental p50 0.284 ms / 215 items vs full 0.544 ms / 2000 items |
| WebGPU / WebGL, WASM, gamepad, pointer lock, audio | not yet | M4/M6 |
| Performance targets (p95 frame time, input latency) | defined | [Gaming platform](gaming.md) |

## Known gaps (honest list)

These are not hidden; they are the M2+ work.

- **The official `wpt run` harness** needs a JS engine and WebDriver and arrives with M4. The reftest runner is the M1 substitute, with its caveats published.
- **Layout is a full-document pass.** Only damage is incremental today.
- **Cascade cost** is 1.94 ms on the 602-node fixture and documented in [Performance baseline](performance.md).

- **No WebGPU/WebGL, WASM, gamepad, pointer lock or low-latency audio yet**; the gaming platform has its contract (ADR-026) and its first slice (UDP), not its rendering APIs.

## Next

Flex wrap, grow/shrink and basis, then grid, tables, floats, images, scrolling, hit testing, layers and stacking contexts. See the milestone table in the decision record.
