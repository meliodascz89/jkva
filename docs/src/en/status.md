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
| Block and inline layout | done | `jkva-layout`; 49 tests; inline boxes, percentages, auto centring, wrapping |
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
| Wrap, grow/shrink/basis, order, align-self | not yet | next M2 increment |
| Grid, tables, floats, images, scroll, hit testing, layers | not yet | M2 |

## Known gaps (honest list)

These are not hidden; they are the M2+ work.

- **The official `wpt run` harness** needs a JS engine and WebDriver and arrives with M4. The reftest runner is the M1 substitute, with its caveats published.
- **Layout is a full-document pass.** Only damage is incremental today.
- **Cascade cost** is 1.94 ms on the 602-node fixture and documented in [Performance baseline](performance.md).

## Next

Flex wrap, grow/shrink and basis, then grid, tables, floats, images, scrolling, hit testing, layers and stacking contexts. See the milestone table in the decision record.
