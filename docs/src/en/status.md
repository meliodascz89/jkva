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
| Block and inline layout | done | `jkva-layout`; 47 tests; text flows and wraps |
| Text shaping and glyph rasterization | done | `jkva-text`, fontdb + harfrust + swash; 21 tests |
| Raster + compositing path | done | `jkva-gfx` scene renderer, alpha blending |
| Screenshot harness with golden pages | done | `examples/hello.html`, `examples/cards.html`, `tools/reftest-runner` |
| WPT reftest harness | done | `tools/wpt-reftest-runner`; first run 10 pass / 7 fail / 7 unsupported of 25 |
| Incremental seam | done | `jkva-shell::Frame` returns a `DamageRegion`; layout is still a full pass |
| Documentation and ADRs current | done | this page, `docs/`, `CHANGELOG.md` |

## Known gaps (honest list)

These are not hidden; they are the M2+ work.

- **Inline element boxes are flattened.** A `<span>` with background, border or padding does not paint its own box yet; its text joins the surrounding inline flow.
- **Percentage margins and padding are not resolved**, and `margin: auto` centering does not exist. Percentage widths and heights are supported.
- **Borders paint in `currentColor` only**; `border-color` is not a tracked property.
- **The official `wpt run` harness** needs a JS engine and WebDriver and arrives with M4. The reftest runner is the M1 substitute, with its caveats published.
- **Layout is a full-document pass.** Only damage is incremental today.
- **Cascade cost** is 1.94 ms on the 602-node fixture and documented in [Performance baseline](performance.md).

## Next: M2

Flex, grid, tables, floats, images, scrolling, hit testing, layers and stacking contexts. See the milestone table in the decision record.
