# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cargo workspace with one crate per engine layer (`jkva-core` … `jkva-shell`).
- `jkva-platform` Metal probe: reports the default device and Apple9 family support.
- Documentation skeleton (mdBook, English and Czech), contributing and security policies.
- CI: macOS format/clippy/test/docs, Linux check for portable crates.
- `tools/reftest-diff`: deterministic pixel diff for reference tests, with diff image output.
- `tools/wpt-report`: Web Platform Tests pass-rate summary and CI gate.
- `tools/wpt-reftest-runner`: discovers WPT reftests, runs them through `jkva-shot` and classifies external-dependency pages as unsupported instead of passing; first run: 10/7/7/1 out of 25.
- `tools/bench-runner`: deterministic micro-benchmarks for HTML parse, CSS parse, the cascade and text shaping, with fixture sizes in every report.
- `tools/reftest-runner`: renders a screenshot corpus with `jkva-shot` and compares it with `reftest-diff`; `examples/hello.html` has a golden image.
- `jkva-core`: generational arena with typed, opaque ids and stale-handle detection.
- `jkva-shell`: `Frame` with the incremental seam — an attribute mutation re-styles, re-lays out and returns a `DamageRegion` covering the changed element and every box that moved or resized.
- `jkva-shell`: paints background colours and borders (currentColor) in addition to glyphs; `examples/cards.html` covers borders, wrapping and monospace.
- `jkva-shell`: the M1 pipeline HTML → cascade → layout → text → Metal, and `jkva-shot` for PNG screenshots; `examples/hello.html` is the first rendered page.
- `jkva-layout`: flex wrapping and grow/shrink/basis — row line breaking, per-line justify/align, basis (including percentages), weighted shrink with a zero floor.
- `jkva-layout`: flex formatting context — row/column, `justify-content`, `align-items`, `align-self`, `gap`, max-content approximation for auto widths and anonymous text items; exclusions documented.
- `jkva-layout`: inline elements produce one background/border box per line, painted behind their text; padding and border still do not advance the inline flow.
- `jkva-layout`: `LengthEdges` for margins and padding; percentage margins and `margin: auto` centring.
- `jkva-layout`: `Length { Auto, Px, Percent }` for widths and heights; percentages resolve against the containing block width.
- `jkva-layout`: text fragments carry font weight and family, measured through `TextMeasure::measure_styled`; collapsible spaces are their own fragments so measurement and painting share one face.
- `jkva-layout`: block and inline layout with auto/explicit widths, margins, padding, sibling margin collapse, whitespace collapsing and greedy line breaking.
- `jkva-style`: `border-*-color` and the `border-color` shorthand; the `border` shorthand sets the colour too.
- `jkva-style`: `border-*-width` and `border-*-style`, with `border-width`, `border-style` and `border` shorthand expansion (border colour is still `currentColor`).
- `jkva-style`: CSS parser, structural selector model, L4 specificity, selector matching and the cascade into `ComputedStyle` (at-rule contents, `var()` and a UA stylesheet are not implemented yet).
- `jkva-text`: font discovery (fontdb), HarfBuzz shaping (harfrust) and glyph rasterization to alpha coverage (swash), with a documented fallback chain.
- `jkva-gfx`: persistent `SceneRenderer` reuses device, queue, pipeline, sampler and white texture; `submit` skips CPU readback and `render_scene` delegates to it.
- `jkva-shell`: `Frame::render_with` / `submit_with` accept a caller-owned renderer for frame loops.
- `jkva-text`: shaped-run cache keyed by text and style, capped at 4096 entries with a wholesale clear.
- `jkva-shell`: glyph rasterization/texture cache and a persistent font cache in `Frame`; two frames with one renderer are identical.
- `engine-bench`: end-to-end paint+submit frame benchmark; the same cards frame dropped from p50 8.05 ms to about 0.51 ms (p95 10.81 to 2.46 ms, median of five runs) after the glyph, font and shaped-run caches.
- `jkva-gfx`: `GpuCapabilities` report (device, buffer and memory budgets, Apple7/8/9 and Metal4, unified memory, timestamp counters) with a cfg-gated test that skips when Metal is unavailable.
- `gfx-bench`: offscreen Metal frame-time benchmark (4096 quads, 1080p, CPU submit / GPU completion / GPU execution p50+p95, JSON).
- `jkva-core`: fixed-timestep `FrameClock` with capped simulation steps, interpolation alpha, dropped-frame reporting and runtime rate changes (ADR-026).
- `jkva-net`: permission-gated UDP transport (Tokio), default deny, per-origin and per-application grants, revoke, datagram-size and byte-budget quotas (ADR-026).
- `jkva-ipc`: protocol v2 adds gamepad input (buttons plus axes) for the gaming platform.
- `jkva-ipc`: versioned, length-prefixed IPC messages with framing checks (ADR-006).
- `jkva-dom`: arena document tree with an html5ever 0.40 `TreeSink`: implied structure, entities, foster parenting, templates, quirks mode and parse diagnostics.
- `jkva-gfx`: Metal 4 device probe, offscreen clear render, MSL triangle, GPU readback (`Image`), display-list rectangle rendering, and a textured scene renderer (alpha blending, linear sampling) for glyphs and images; `gfx-clear`, `gfx-triangle` and `gfx-dl` PNG probes.
- The decision record: 99 answered questions and 24 ADRs.
