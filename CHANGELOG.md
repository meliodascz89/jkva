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
- `jkva-core`: generational arena with typed, opaque ids and stale-handle detection.
- `jkva-style`: CSS parser, structural selector model, L4 specificity and structured warnings (at-rule contents, nesting and `var()` are not implemented yet).
- `jkva-dom`: arena document tree with an html5ever 0.40 `TreeSink`: implied structure, entities, foster parenting, templates, quirks mode and parse diagnostics.
- `jkva-gfx`: Metal 4 device probe, offscreen clear render, MSL triangle, GPU readback (`Image`), and display-list rectangle rendering; `gfx-clear`, `gfx-triangle` and `gfx-dl` PNG probes.
- The decision record: 99 answered questions and 24 ADRs.
