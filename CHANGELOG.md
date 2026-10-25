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
- `tools/bench-runner`: deterministic micro-benchmarks for HTML parse, CSS parse, the cascade and text shaping, with fixture sizes in every report.
- `tools/reftest-runner`: renders a screenshot corpus with `jkva-shot` and compares it with `reftest-diff`; `examples/hello.html` has a golden image.
- `jkva-core`: generational arena with typed, opaque ids and stale-handle detection.
- `jkva-shell`: paints background colours and borders (currentColor) in addition to glyphs; `examples/cards.html` covers borders, wrapping and monospace.
- `jkva-shell`: the M1 pipeline HTML → cascade → layout → text → Metal, and `jkva-shot` for PNG screenshots; `examples/hello.html` is the first rendered page.
- `jkva-layout`: text fragments carry font weight and family, measured through `TextMeasure::measure_styled`; collapsible spaces are their own fragments so measurement and painting share one face.
- `jkva-layout`: block and inline layout with auto/explicit widths, margins, padding, sibling margin collapse, whitespace collapsing and greedy line breaking.
- `jkva-style`: `border-*-width` and `border-*-style`, with `border-width`, `border-style` and `border` shorthand expansion (border colour is still `currentColor`).
- `jkva-style`: CSS parser, structural selector model, L4 specificity, selector matching and the cascade into `ComputedStyle` (at-rule contents, `var()` and a UA stylesheet are not implemented yet).
- `jkva-text`: font discovery (fontdb), HarfBuzz shaping (harfrust) and glyph rasterization to alpha coverage (swash), with a documented fallback chain.
- `jkva-dom`: arena document tree with an html5ever 0.40 `TreeSink`: implied structure, entities, foster parenting, templates, quirks mode and parse diagnostics.
- `jkva-gfx`: Metal 4 device probe, offscreen clear render, MSL triangle, GPU readback (`Image`), display-list rectangle rendering, and a textured scene renderer (alpha blending, linear sampling) for glyphs and images; `gfx-clear`, `gfx-triangle` and `gfx-dl` PNG probes.
- The decision record: 99 answered questions and 24 ADRs.
