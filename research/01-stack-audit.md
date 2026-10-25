# Crate stack audit for a from-scratch browser rendering engine

**Date:** 2026-10-25 (Europe/Prague)
**Target:** arm64 macOS (Apple Silicon), Metal, Rust edition 2024, permissive open-source project
**Question:** which crates should the engine be built on, which are traps, and what does the ecosystem actually look like today?

**Method.** Every version, release date and licence in this document was read from the crates.io API (`https://crates.io/api/v1/crates/<name>`) on 2026-10-25, not from memory. Maintenance was measured two ways: last *release* date (from crates.io) and last *commit* date (from the repository's `commits.atom` feed, which is not rate-limited like the REST API). Production usage was measured by grepping the actual `Cargo.lock` files of Servo, Firefox and Ladybird. Claims I could not verify in this session are marked **unverified** rather than smoothed over.

**Recency rule used throughout.** "Active" = a release on or after **2026-04-25** (the six-month window). "Stale release" = no release in 6–24 months. "Dead end" = no release **and** no commits for more than 24 months, or an explicit upstream abandonment notice.

---

## 0. Bottom line up front

Four findings should change the plan more than any individual crate choice:

1. **`rustybuzz` is archived.** Its own README says "This project is not developed further, unmaintained, and archived. We recommend that all users switch to HarfRust instead." ([rustybuzz README](https://github.com/harfbuzz/rustybuzz)). Both Servo and `cosmic-text` have already migrated to `harfrust`. Any plan that names rustybuzz as the shaper is starting on a dead crate.
2. **`html5ever`, `markup5ever` and `tendril` are MIT OR Apache-2.0, not MPL-2.0.** The brief asserts they are MPL-2.0; that is wrong and it matters. Only `cssparser`, `selectors`, `stylo*`, `webrender` and `mozjs` are MPL-2.0. ([html5ever workspace Cargo.toml](https://github.com/servo/html5ever/blob/main/Cargo.toml))
3. **Servo's style engine *is* published** — as the `stylo` family (`stylo`, `stylo_dom`, `stylo_traits`, `stylo_static_prefs`), version **0.22.0** (2026-09-30), MPL-2.0, from `servo/stylo`. The crates.io crate literally named `style` is an **unrelated, dead 2019 crate** — do not confuse them. Blitz consumes the `stylo` family today.
4. **Vello was restructured in 2026.** The crate named `vello` (0.11.0) is now the *research* compute renderer — explicitly "experimental" in its own README. The production-intended renderers are `vello_cpu` 0.3.0 (most mature) and `vello_gpu` 0.3.0, with `glifo` 0.4.0 for text. ([vello README](https://github.com/linebender/vello), [releases](https://github.com/linebender/vello/releases))

The practical consequence: the highest-leverage decision in this stack is not "which graphics API" — it is **whether to adopt Stylo (MPL-2.0, a six-figure-LOC CSS cascade you cannot write yourself in a year) or write CSS cascade from scratch on top of `cssparser`**. Everything else is replaceable.

---

## 1. Master table

Verdicts: **pick** = adopt now · **watch** = viable later or conditional · **reject** = do not use.

### HTML & CSS parsing

| crate | latest | released | licence | maintenance | macOS arm64 | maturity / spec | key limitation | verdict |
|---|---|---|---|---|---|---|---|---|
| `html5ever` | 0.40.1 | 2026-09-14 | MIT OR Apache-2.0 | Active (1.3 mo; commits 2026-09-14) | Yes (pure Rust) | The reference Rust HTML5 tree builder; html5lib-test-driven; used by Servo + Blitz | Tree construction only — no CSS, no scripting, no fragment cache; `web_atoms`/`string_cache` overhead on hot paths | **pick** |
| `markup5ever` | 0.40.0 | 2026-09-11 | MIT OR Apache-2.0 | Active (1.4 mo) | Yes | Shared infra for html5ever/xml5ever (qualnames, tree sinks) | Transitive; version-locked to html5ever | **pick** (transitive) |
| `tendril` | 0.5.1 | 2026-07-06 | MIT OR Apache-2.0 | Active (3.6 mo; commits 2026-09-14) | Yes | UTF-8/UTF-16 rope-ish string type used by html5ever/cssparser | Opaque API, `unsafe` internals, historically the least-loved Servo crate; hard to replace once embedded | **pick** (transitive) |
| `lol_html` | 3.0.2 | 2026-10-23 | BSD-3-Clause | Active (0.1 mo) | Yes | Cloudflare production streaming rewriter; fast | Not a DOM: streaming transformation only, no tree, no CSSOM | **watch** (rewriting/proxy only) |
| `scraper` | 0.29.0 | 2026-10-21 | ISC | Active (0.1 mo) | Yes | Selector queries over html5ever for scraping | Wrong tool for an engine: no cascade, no layout, allocates per query | **reject** |
| `cssparser` | 0.38.0 | 2026-09-07 | **MPL-2.0** | Active (1.6 mo; commits 2026-10-08; edition 2024) | Yes | The CSS tokeniser/parser Servo + Firefox both ship | Token/parser only — no cascade, no typed values, no invalidation | **pick** (accept MPL) |
| `selectors` | 0.41.0 | 2026-09-30 | **MPL-2.0** | Active (0.8 mo; commits 2026-10-22; edition 2024) | Yes | Servo/Firefox selector matching incl. bloom-filter fast paths | Generic over your DOM via traits; you must implement `Element`/`SelectorImpl` correctly; no style invalidation | **pick** (accept MPL) |
| `lightningcss` | 1.0.0-alpha.72 | 2026-07-20 | **MPL-2.0** | Active (3.2 mo; commits 2026-10-08) | Yes | Excellent *bundler/minifier* (npm `lightningcss` 1.33.0, same day) | **No stable Rust API after 5 years of alphas**; visitor-based transformer, not a browser cascade; forked `parcel_selectors` | **reject** (engine core); watch for tooling |
| `stylo` + `stylo_dom`/`stylo_traits`/`stylo_static_prefs` | 0.22.0 | 2026-09-30 | **MPL-2.0** | Active (0.8 mo; commits 2026-10-22) | Yes (Servo, Firefox, Blitz all build it) | The actual Firefox/Servo CSS cascade + animation + invalidation. Production-grade. | Huge; demands you implement `TDocument`/`TElement`/`TShadowRoot` and a layout bridge (`stylo_taffy` — 0.2.0 stable on crates.io, Blitz pins `=0.3.0-beta.4`); MPL file-level copyleft; API churn every release (0.19→0.22 in 3 months) | **pick** (the big bet) |
| `style` (crates.io) | 0.1.0 | 2019-12-21 | MIT/Apache-2.0 | **Dead** (82 mo, no repo) | n/a | Nothing — unrelated to Servo | Not Servo's `style`; a 2019 placeholder. Servo's real `style` crate is **not published** under that name | **reject** |
| cssparser-based alternatives | — | — | — | — | — | Only two real options: Stylo (full cascade) or hand-rolled cascade on `cssparser` + `selectors` (+ `swc_css`, `parcel_selectors`, `css-inline` as niches) | Hand-rolling cascade is a 6–18 month project on its own | — |

### Layout

| crate | latest | released | licence | maintenance | macOS arm64 | maturity / spec | key limitation | verdict |
|---|---|---|---|---|---|---|---|---|
| `taffy` | 0.15.0 | 2026-10-12 | MIT | Active (0.4 mo; commits 2026-10-21) | Yes | Flexbox + **grid** + block + float + calc, content sizing, `parse` via cssparser. Used by Blitz and (as of 0.15) Servo's `servo-layout` | No inline/text-flow model, no fragmentation/pagination, no tables (Blitz emulates tables with grid), no `position: sticky`; block/float layout is new in 0.15 | **pick** |
| `yoga` (`yoga-rs`) | 0.5.0 | 2025-01-16 | MIT | **Dormant** (21.3 mo since release; 2 commits on 2026-10-12 after a 20-month gap; 163 stars) | Yes via C++ build (`cc`) | Facebook Yoga: flexbox only | **Flexbox only — no grid.** C++ build step, thin/abandoned bindings, no spec tracking | **reject** |
| Servo layout crates (`layout_2020`, `script`) | unpublished | — | MPL-2.0 | Servo-internal | Servo builds on arm64 macOS | Real fragment/inline/table layout | **Not on crates.io and not designed as a library** — deeply coupled to Servo's script/DOM. Only `servoshell` 0.7.0 (2026-10-05) is published, and it is the whole browser | **reject** (unavailable) |
| `blitz-dom` | 0.3.0-beta.4 | 2026-10-16 | MIT OR Apache-2.0 | Active (0.3 mo; commits 2026-10-24) | Yes (wgpu/Vello; macOS builds on blitz.is/downloads) | Integrates html5ever + cssparser + selectors + **stylo 0.22** + taffy 0.15 + parley 0.12 + skrifa 0.44 + icu 2.3. Best existing proof that the pieces compose. | Beta; no JS at all; `display: table` partial (grid emulation), subgrid no, `overflow: auto` no, `position: sticky` in progress, no 3D transforms ([status page](https://blitz.is/status/css)) | **pick** (as reference architecture and possible foundation) |
| `blitz` | 0.3.0-beta.4 | 2026-10-16 | MIT OR Apache-2.0 | Active (0.3 mo) | Yes | App-level shell over blitz-dom + anyrender | Beta, opinionated, "don't want the entirety of browser features" | **watch** |
| `vello` + `lazy` | see Graphics | — | — | — | — | `lazy` on crates.io is a **dead 2015 crate** (0.5.3, 139 mo). If the brief meant "lazy rendering", no such crate exists — invalidation/lazy paint is yours to write | — | **reject** `lazy`; see Vello below |

### Text & fonts

| crate | latest | released | licence | maintenance | macOS arm64 | maturity / spec | key limitation | verdict |
|---|---|---|---|---|---|---|---|---|
| `rustybuzz` | 0.20.1 | 2024-11-12 | MIT | **ARCHIVED — dead end** (23.4 mo since release; README states unmaintained) | Yes | HarfBuzz 10.1.0 parity, 2221/2252 shaping tests | Superseded; no further fixes; ttf-parser-based | **reject** |
| `harfrust` | 0.15.0 | 2026-10-22 | MIT | Active (0.1 mo) | Yes | Rust port of HarfBuzz matching **HarfBuzz 15.0.0**; passes most of HarfBuzz's test+fuzz suites; **<25% slower than HarfBuzz** ([README](https://github.com/harfbuzz/harfrust)). Used by Servo (0.12.0) and cosmic-text (0.5) | Known conformance gaps vs HarfBuzz (see `HARFBUZZ.md`); no graphite; malformed fonts error instead of falling back; no size property (UnitsPerEm only, you scale) | **pick** |
| `fontdb` | 0.24.0 | 2026-07-29 | MIT | Active (2.9 mo) | Yes (CoreText/system font paths) | Font enumeration + fallback; used by Servo and resvg | On macOS you will likely want CoreText enumeration for system fonts; `fontdb`'s macOS scan is generic | **pick** |
| `swash` | 0.2.10 | 2026-07-17 | Apache-2.0 OR MIT | Active (3.3 mo) | Yes | Font introspection, shaping (own engine), scaling, colour/bitmap glyphs, hinting | Overlaps `skrifa`+`harfrust`; picking both duplicates caches. Used by `cosmic-text` | **watch** (only for colour-glyph/COLR bitmap paths) |
| `allsorts` | 0.17.0 | 2026-05-13 | Apache-2.0 | Active (5.4 mo, borderline) | Yes | Pure-Rust OpenType parser + subsetter (Prince XML heritage) | Duplicate of fontations (`skrifa`/`read-fonts`) capabilities; subsetting is the only unique draw; smaller community | **watch** |
| `cosmic-text` | 0.19.0 | 2026-04-22 | MIT OR Apache-2.0 | Active (6.1 mo release, commits 2026-09-30) | Yes | Full text stack (fontdb + harfrust 0.5 + skrifa 0.40 + swash + bidi + linebreak), used by COSMIC desktop | **Editor-oriented**, not CSS-oriented: no CSS `line-height`/`word-spacing`/`text-indent`/justification-per-CSS, no `::first-line`, no shaping across inline boxes. Its own `Attrs` model | **watch** (shortest path to "text on screen") |
| `parley` | 0.12.0 | 2026-10-09 | Apache-2.0 OR MIT | Active (0.5 mo; commits 2026-10-22) | Yes | Linebender rich-text layout; now split `fontique` (font enumeration/fallback) + `parley_engine` + `parlance`. Used by Blitz | Younger than cosmic-text; CSS-specific features (justify, hyphenation policies) must be driven by you; API churn (0.12 restructure) | **pick** |
| `skrifa` / `read-fonts` (fontations) | 0.49.0 / 0.46.0 | 2026-10-21 | MIT OR Apache-2.0 | Active (0.1 mo) | Yes | Google's fontations: `read-fonts` parses, `skrifa` outlines/scales/metrics/colour. Used by **Firefox** (skrifa 0.44, read-fonts 0.41), Servo (0.40), Blitz, harfrust | No shaping (by design — pair with harfrust); hinting coverage narrower than FreeType/CoreText (`skrifa` has a hinting engine but it is not byte-exact vs macOS) | **pick** |
| `unicode-bidi` | 0.3.18 | 2024-12-16 | MIT OR Apache-2.0 | Active repo (commits 2026-09-15) but **stale release** (22.3 mo) | Yes | UAX #9 reference-quality; used by Servo, Firefox, cosmic-text | Slow release cadence; hardcodes UCD tables per release (bidi class changes lag Unicode) | **pick** |
| `icu4x` (`icu`, `icu_segmenter`, `icu_properties`, `icu_locale_core`, `icu_casemap`) | 2.4.0 | 2026-10-22 | **Unicode-3.0** (permissive, OSI-approved) | Active (0.1 mo) | Yes | ICU4X 2.x: segmentation (UAX #29), properties, normalisation, locale, case mapping, collation. **Firefox ships `icu_segmenter` 2.1.2**; Blitz uses `icu_*` 2.3 | Data-provider plumbing (`icu_provider`/`compiled_data`) adds binary size; not a drop-in for `Intl` (that is your JS engine's job) | **pick** |

### Graphics & compositing

| crate | latest | released | licence | maintenance | macOS arm64 | maturity / spec | key limitation | verdict |
|---|---|---|---|---|---|---|---|---|
| `wgpu` | 30.0.1 | 2026-08-22 | MIT OR Apache-2.0 | Active (2.1 mo; commits 2026-10-25) | **Yes — Metal backend on by default**; MSRV 1.87 | "The core of the WebGPU integration in **Firefox, Servo, and Deno**" ([docs](https://docs.rs/wgpu/latest/wgpu/)); WebGPU spec surface + extensions | Thin abstraction, not a renderer: no glyphs, no paths, no compositor. Version churn is brutal (v29→v30 landed 2026-07-01, breaking). Metal limits: combined buffer-argument-table limit of 29 per stage; sample-array elements reduced to 996 on Apple7/8/Mac2 | **pick** |
| `vello` (research compute renderer) | 0.11.0 | 2026-10-02 | Apache-2.0 OR MIT | Active (0.8 mo) | Yes, via wgpu 30 + Metal | Compute-shader vector rasteriser; the "GPU text" path is `Scene::draw_glyphs(&FontData)` → glyph outlines from `skrifa` 0.44, not SDF/MSDF | **Now labelled experimental by upstream**; requires compute + indirect execution (fails on iOS Simulator per wgpu changelog); no CPU fallback | **watch** (only via `anyrender_vello`) |
| `vello_gpu` | 0.3.0 | 2026-10-02 | Apache-2.0 OR MIT | Active (0.8 mo) | Yes (wgpu default backends incl. Metal; optional WebGL2) | Sparse-Strips: CPU preprocessing + GPU raster/composite; `text` feature on by default exposing `Scene::glyph_run`; glyph **atlas caching** when `full_image_support` is on ([README](https://github.com/linebender/vello/tree/main/vello_gpu)) | Deliberately no compute-shader requirement, so it is the *compatible* one, but it is newer than `vello_cpu` and upstream says Vello CPU is "currently overall more mature" | **pick** (target this, not `vello`) |
| `vello_cpu` | 0.3.0 | 2026-10-02 | Apache-2.0 OR MIT | Active (0.8 mo) | Yes (pure CPU, SIMD) | **"The most mature choice"** per upstream README; Sparse Strips on CPU, multithreaded + SIMD | CPU-only: no GPU compositing, so animations/scroll on 4K need your own tiling; text via `glifo` 0.4.0 | **pick** (fallback + tests) |
| `glifo` | 0.4.0 | 2026-10-02 | Apache-2.0 OR MIT | Active (0.8 mo) | Yes | Glyph-run/text support shared by both Sparse Strips renderers | 0.x, API churn, thin docs | **pick** (with vello_*) |
| `lyon` | 1.0.19 | 2026-03-08 | MIT OR Apache-2.0 | Active repo (commits 2026-10-23) but **stale release** (7.6 mo) | Yes | Best-in-class path tessellation (fill/stroke, round joins, dashes) | Tessellation, not rendering — needs wgpu/your own pipelines; no anti-aliasing strategy beyond per-vertex coverage | **pick** (if not using Vello) / watch |
| `tiny-skia` | 0.12.0 | 2026-02-02 | BSD-3-Clause | Stale release (8.7 mo), active repo (2026-08-20) | Yes | Skia-subset CPU rasteriser (by resvg's author) | CPU only; no GPU; slower than Skia; limited blend modes | **pick** (for tests/golden images and paint fallback) |
| `resvg` | 0.49.0 | 2026-10-24 | Apache-2.0 OR MIT | Active (0.0 mo) | Yes | Best Rust SVG renderer (used by Servo 0.48.1) | SVG only — not a page renderer; drives `usvg` + `tiny-skia`; no CSS cascade inside SVG beyond what `usvg` supports | **pick** (for `<svg>`/SVG images) |
| `femtovg` | 0.27.0 | 2026-08-31 | MIT OR Apache-2.0 | Active (1.8 mo; commits 2026-10-23) | Yes but **via OpenGL** (deprecated on macOS) | NanoVG-style immediate-mode 2D on GL | GL-only; macOS OpenGL is deprecated/software-ish on Apple Silicon; no Metal path | **reject** (for this target) |
| `piet` | 0.8.0 | 2025-08-29 | Apache-2.0 OR MIT | **Legacy** (13.9 mo release; commits 2026-05-02) | Yes | Linebender's pre-Vello 2D abstraction | Superseded by Vello by the same org; no new work | **reject** |
| `objc2-metal` | 0.3.2 | 2025-10-04 | Zlib OR Apache-2.0 OR MIT | Slow per-crate release (12.7 mo) but repo very active (commits 2026-10-17); **this is what wgpu-hal 30 and Blitz and Firefox use** | Native Metal | Generated, safe-ish bindings to the whole Metal framework | Slow release cadence for this one crate; `unsafe` at the boundary; you own command-buffer lifetime | **pick** (if bypassing wgpu) |
| `metal` (gfx-rs) | 0.33.0 | 2025-12-17 | MIT OR Apache-2.0 | Stale release (10.2 mo), active repo | Native Metal | Older hand-written Metal bindings | **wgpu-hal 30 moved to `objc2-metal`** — this crate is the legacy path; less complete framework coverage | **watch** |
| `webrender` (bonus) | 0.70.0 | 2026-07-10 | **MPL-2.0** | Released from servo/servo; the standalone repo's commit feed is stale (2025-10-07) | Yes (Servo nightly arm64) | The production compositor of Servo *and* Firefox (Firefox vendors it in-tree as `webrender`, `webrender_api`, `wr_glyph_rasterizer`) | Enormous, consumes a very specific display-list API, tied to Servo's `webrender_api` types; adopting it means adopting Servo's whole paint model; MPL | **watch** (do not plan on it) |

### JavaScript engines

| crate | latest | released | licence | maintenance | macOS arm64 | maturity / spec | key limitation | verdict |
|---|---|---|---|---|---|---|---|---|
| `boa_engine` | 0.22.0 | 2026-08-28 | Unicode-3.0 OR MIT | Active (1.9 mo; commits 2026-10-17) | Yes (pure Rust) | "Experimental … more than **90%** of the latest ECMAScript specification" ([README](https://github.com/boa-dev/boa)); Test262 dashboard at boajs.dev/conformance | **No JIT, no generational GC integration hooks**, 10–100× slower than V8/JSC on real JS; no `Intl` depth; embedding API churns every release | **watch** (prototype/CI only) |
| `deno_core` | 0.413.0 | 2026-10-21 | MIT (V8 itself BSD-3-Clause) | Active (0.1 mo; commits 2026-10-25) | Yes — prebuilt V8 static libs for aarch64-apple-darwin, downloaded at build time ([rusty_v8](https://github.com/denoland/rusty_v8)) | V8 = spec leader. Production in Deno. | Runtime-shaped (op system, `JsRuntime`, module loader, `OpState`); GC is V8's, with `v8::Weak`/`TracedReference` for host handles — **no precise integration with a Rust GC**; every DOM call goes through an op/`FunctionTemplate` boundary; ~100 MB+ binary and long link times | **watch → pick later** (see §4) |
| `mozjs` | 0.26.10 | 2026-10-24 | **MPL-2.0** | Very active (0.0 mo; commits 2026-10-25) | Yes — Servo ships arm64 macOS with it; prebuilt archives with GitHub attestation ([README](https://github.com/servo/mozjs)) | SpiderMonkey tracking **mozilla-esr153 (153.0)**, "battle-tested in Servo" | MPL-2.0 (file-level copyleft); heavy C++ build unless using prebuilt archive; bindings are `unsafe` and C++-shaped (`JSContext`, `Rooted`, `HandleObject`) | **pick** (for a real engine) |
| `rquickjs` | 0.14.0 | 2026-09-18 | MIT | Active (1.2 mo; commits 2026-10-20) | Yes (bundled QuickJS via `cc`) | QuickJS: ES2023-ish, small, no JIT | Interpreter-speed only (~10–20× slower than JSC); no JIT; limited `Intl`; small community | **watch** (embedding scripts, not page JS) |
| `javascriptcore-rs` | 2.0.0 | 2026-09-22 | MIT | Active (1.1 mo; commits 2026-09-22) | **Yes — system JavaScriptCore, zero build cost** | JSC is the Safari engine; Tauri-maintained bindings (`tauri-apps/javascriptcore-rs`) | Bindings are WKWebView/Tauri-shaped (JSContext/JSValue wrappers), **not** a browser embedding API: no `JSObjectSetPrivate`-style GC rooting protocol exposed, no same-thread DOM hooks, macOS-only (needs `JavaScriptCore.framework`) | **watch** (fastest prototype path, wrong ceiling) |
| `v8` (rusty_v8) | 152.2.0 | 2026-08-20 | MIT | Active (2.2 mo; commits 2026-10-21) | Yes — prebuilt static lib per version | V8 bindings; use through `deno_core` in practice | Raw bindings are extremely `unsafe`; you would be reimplementing `deno_core`'s plumbing; V8 152 is a large moving target | **watch** (via deno_core only) |

### WebAssembly

| crate | latest | released | licence | maintenance | macOS arm64 | maturity / spec | key limitation | verdict |
|---|---|---|---|---|---|---|---|---|
| `wasmtime` | 50.0.0 | 2026-10-20 | Apache-2.0 **WITH LLVM-exception** | Active (0.2 mo; commits 2026-10-23) | Yes (Cranelift JIT, aarch64) | Reference Wasm implementation; full proposal support, WASI, component model | Very large dependency; JIT needs W^X (fine on macOS with hardened runtime + `com.apple.security.cs.allow-jit`); ~monthly breaking releases | **pick** |
| `wasmer` | 7.5.0 | 2026-10-01 | MIT | Active (0.8 mo) | Yes (LLVM/Cranelift/singlepass) | Mature, multiple backends | Heavier, commercial features gate some paths; runtime embedding API less stable than wasmtime | **watch** |
| `wasmi` | 2.0.0 | 2026-09-01 | MIT/Apache-2.0 | Active (1.8 mo) | Yes (interpreter, no JIT) | Clean, small, fast interpreter; good wasm→wasm and no-unsafe story | Interpreter only: 5–20× slower than JIT — acceptable for CSS `@supports`-style sandbox work, not for media codecs | **pick** (small/fast-to-build) or wasmtime |
| `wasm3` | 0.3.1 | 2021-09-19 | MIT | **DEAD** (61 mo since release; last commit 2022-03-04) | n/a | Interpreter | Unmaintained for 4.5 years; no SIMD/threads/reference-types | **reject** |

### Networking

| crate | latest | released | licence | maintenance | macOS arm64 | maturity / spec | key limitation | verdict |
|---|---|---|---|---|---|---|---|---|
| `hyper` | 1.12.0 | 2026-10-06 | MIT | Active (0.6 mo) | Yes | HTTP/1.1 + HTTP/2 client/server core; **Firefox ships hyper 1.9.0** | Low-level: no TLS, no redirects, no cookies, no cache — you assemble those | **pick** |
| `h2` | 0.4.20 | 2026-10-06 | MIT | Active (0.6 mo) | Yes | HTTP/2 state machine (used by hyper) | Protocol only | **pick** (via hyper) |
| `h3` | 0.0.8 | 2025-05-06 | MIT | Repo active (commits 2026-09-20) but **no release in 17.6 mo** | Yes | HTTP/3 over quinn | Still `0.0.x` after years; API unstable; **Firefox does not use it** (uses `neqo` 0.33.0) | **watch** |
| `quinn` | 0.11.12 | 2026-09-14 | MIT OR Apache-2.0 | Active (1.3 mo; commits 2026-10-23) | Yes | Best Rust QUIC; used by h3, reqwest | Not what Firefox ships (neqo), so HTTP/3 interop bugs are yours | **pick** |
| `rustls` | 0.23.46 | 2026-10-14 | Apache-2.0 OR ISC OR MIT | Active (0.4 mo) | Yes (`ring`/`aws-lc-rs` backends both support aarch64-darwin) | Memory-safe TLS 1.2/1.3; **Servo ships it** | No TLS 1.0/1.1; certificate-verification policy is yours (CT, OCSP, name constraints); `aws-lc-rs` needs CMake at build time (use `ring`) | **pick** |
| `reqwest` | 0.13.5 | 2026-09-08 | MIT OR Apache-2.0 | Active (1.5 mo) | Yes | Batteries-included HTTP client | It is a *client library*, not a browser network stack: you must bypass it for credentials/modes/cache/`fetch` semantics; pulls tokio+hyper+rustls | **watch** (tests/tooling only) |
| `tokio` | 1.54.1 | 2026-10-21 | MIT | Active (0.1 mo) | Yes | The async runtime | Not a browser event loop: you will need a main-thread loop + `LocalSet`; tokio cannot own the platform run loop (winit/NSApplication) | **pick** (network/IO threads only) |
| `http-cache-semantics` | 3.0.0 | 2026-02-04 | BSD-2-Clause | Stale release (8.6 mo), commits 2026-05-21 | Yes | RFC 9111 cache policy decisions; used by reqwest | Policy only — you supply the store, revalidation, and `Vary` keying; no partitioned/`Cache-Control: private` browser semantics | **pick** |
| `cookie_store` | 0.22.1 | 2026-02-16 | MIT OR Apache-2.0 | Stale release (8.2 mo, no commits since) | Yes | RFC 6265 cookie jar; used by reqwest | No `SameSite` enforcement history, no partitioned cookies (CHIPS), no cookie-policy/ITP heuristics — browsers need all of that; single-maintainer | **watch** (or write your own jar) |
| `base64` | 0.23.1 | 2026-08-04 | MIT OR Apache-2.0 | Active (2.7 mo) | Yes | Standard, SIMD-accelerated, audited | 0.22→0.23 migration churn (API-breaking every ~year) | **pick** |
| `simd-json` | 0.18.1 | 2026-08-23 | Apache-2.0 OR MIT | Active (2.1 mo; commits 2026-10-25) | Yes (NEON via runtime detection) | Fastest Rust JSON, simdjson port | Requires mutable input buffer / `unsafe`; needs target-feature detection; **younger than `serde_json` and no `no_std`**; for `<script type=json>` payloads the safety/robustness tradeoff is questionable | **watch** |

---

## 2. Corrections to the brief (verified)

| Brief says | Reality | Evidence |
|---|---|---|
| "html5ever/cssparser/servo crates are MPL-2.0" | `html5ever` 0.40.1, `markup5ever` 0.40.0, `tendril` 0.5.1 are **MIT OR Apache-2.0**. `cssparser`, `selectors`, `stylo*`, `webrender`, `mozjs` are **MPL-2.0**. | [html5ever workspace Cargo.toml](https://github.com/servo/html5ever/blob/main/Cargo.toml); [cssparser Cargo.toml](https://github.com/servo/rust-cssparser/blob/main/Cargo.toml) |
| "servo's style crate (published or not?)" | Published, but as **`stylo` 0.22.0** + `stylo_dom`/`stylo_traits`/`stylo_static_prefs` (all 2026-09-30). The crates.io crate named `style` is an unrelated dead 2019 crate. | crates.io API for `stylo`, `style`; [servo/stylo](https://github.com/servo/stylo) |
| "rustybuzz" as a candidate | **Archived.** Upstream's own README redirects to HarfRust. Servo's lock has `harfrust 0.12.0` and no rustybuzz. | [rustybuzz README](https://github.com/harfbuzz/rustybuzz); [Servo Cargo.lock](https://github.com/servo/servo/blob/main/Cargo.lock) |
| "vello" as one thing | Split into `vello` (research compute, "experimental"), `vello_cpu` 0.3.0 ("most mature"), `vello_gpu` 0.3.0, plus `glifo` 0.4.0 and `vello_common` 0.3.0 — all released 2026-10-02. | [vello README](https://github.com/linebender/vello), [releases](https://github.com/linebender/vello/releases) |
| "lightningcss" | Still **`1.0.0-alpha.72`** on crates.io after ~5 years of alphas (npm is 1.33.0). No stable Rust API. | crates.io + [npm registry](https://registry.npmjs.org/lightningcss) |
| "yoga (yoga-rs)" | Release 0.5.0 from **2025-01-16**, two commits in Oct 2026 after a 20-month silence; flexbox only. | crates.io; [bschwind/yoga-rs](https://github.com/bschwind/yoga-rs) |
| `icu4x` | Licensing is **Unicode-3.0**, not MIT/Apache — permissive and OSI-approved, but your licence scanner will flag it; it is fine for a permissive project. | crates.io `icu` 2.4.0 |

---

## 3. Q1 — who actually ships these in production?

Measured by reading real `Cargo.lock` files, not blog posts.

### Servo (`servo/servo` main, 2026-10-25, 1154 packages)
Ships: `html5ever 0.40.1`, `markup5ever 0.40.0`, `tendril 0.5.1`, `cssparser 0.38.0`, `selectors 0.41.0`, `stylo 0.22.0`, `webrender 0.70.0`, `wgpu 30.0.1`, **`vello 0.11.0` + `vello_cpu 0.2.0`** (canvas), `taffy 0.15.0` (in `servo-layout`), `harfrust 0.12.0`, `fontdb 0.24.0`, `skrifa 0.40.0`, `read-fonts 0.37.0`, `unicode-bidi 0.3.18`, `mozjs 0.26.9`, `hyper 1.12.0`, `h2 0.4.20`, `rustls 0.23.46`, `tokio 1.54.1`, `tiny-skia 0.11.4`, `resvg 0.48.1`, `objc2-metal 0.2.2`.
Does **not** ship: `rustybuzz`, `swash`, `allsorts`, `cosmic-text`, `lol_html`, `scraper`, `boa`, `simd-json`, `cookie_store`, `http-cache-semantics`, `quinn`, `wasmtime`, `piet`, `femtovg`, `metal`.
Notable: Servo has **moved to `harfrust`**, adopted **`taffy`**, and now uses **Vello for canvas** alongside WebRender.

### Firefox (`mozilla-firefox/firefox` main, 909 packages)
Firefox is the most instructive case because it **does not consume Servo crates from crates.io at all** — it vendors or git-pins them:

| dep | version | how Firefox gets it |
|---|---|---|
| `style`, `style_traits`, `style_derive` | 0.0.1 | **path / in-tree** (vendored copy) |
| `webrender`, `webrender_api`, `wr_glyph_rasterizer`, `webrender_bindings` | **0.62.0** | **path / in-tree** — note: older than crates.io's 0.70.0 |
| `selectors` | 0.26.0 | **path / in-tree** — older than crates.io's 0.41.0 |
| `cssparser` | 0.38.0 | **git pin** `servo/rust-cssparser?rev=9f3cedd` |
| `unicode-bidi` | 0.3.15 | **git pin** `servo/unicode-bidi?rev=ca612da` |
| `base64` | 0.21.999 | **path / in-tree** (the `.999` suffix is Firefox's vendoring convention) |
| `skrifa` / `read-fonts` | 0.44.0 / 0.41.0 | registry (crates.io) |
| `icu_segmenter` | 2.1.2 | registry |
| `objc2-metal` | 0.3.2 | registry |
| `hyper` / `h2` / `tokio` | 1.9.0 / 0.4.16 / 1.52.3 | registry |
| `wgpu` | git pin | `git+https://github.com/gfx-rs/wgpu?rev=e084b66…`, `naga 30.0.0`, via `wgpu_bindings` |

So Firefox's Stylo *is* Servo's code, synced by Bugzilla bugs (e.g. [bug 2070964, "Upstream Stylo changes from Servo (Aug 2026)"](https://bugzilla.mozilla.org/show_bug.cgi?id=2070964), which notes "cssparser was upgraded, and the style crate was changed to 2024 edition"). QUIC is **`neqo 0.33.0`**, not quinn. No `rustybuzz`, `html5ever`, `vello`, `taffy`, or `rustls`.

The lesson for us: even Mozilla, with full-time engineers, treats these crates as **source it vendors**, not as semver-stable dependencies. That is the realistic maintenance model for Stylo/WebRender consumers.

### Ladybird (`LadybirdBrowser/ladybird` master, 188 Rust packages)
Ladybird's Rust workspace is **almost entirely self-authored**: `LibWasm/Engine`, `LibJS/JIT/Rust`, `LibRegex/Rust`, `LibUnicode/Rust`, `LibWeb/HTML/Parser/Rust`, `LibIPC/Rust`, `LibGfx/Rust`, `RustAllocator`. The **only** crate from this audit's candidate list in its lock file is `base64 0.22.1`. Ladybird is the strongest counter-example to "reuse the ecosystem": they wrote their own Wasm engine rather than adopt wasmtime/wasmi.

### Blitz / Dioxus (`dioxuslabs/blitz`, blitz-dom 0.3.0-beta.4 dependencies)
`html5ever`/`markup5ever 0.40`, `cssparser 0.38`, `selectors 0.41`, **`stylo`/`stylo_dom`/`stylo_traits`/`stylo_static_prefs` 0.22**, `stylo_taffy`, `taffy 0.15`, `parley 0.12`, `skrifa 0.44`, `icu_* 2.3`, `kurbo 0.13`, `objc2 0.6`, `anyrender 0.14` (+ `anyrender_vello 0.15` → `vello 0.11` + `wgpu 30`; `anyrender_vello_cpu 0.18` → `vello_cpu 0.3` + `glifo 0.4`), `usvg 0.48`.
This is the single most useful existence proof in the ecosystem: **the combination is known to compile and render** on macOS.

### Chromium-adjacent
Chromium itself uses Skia (C++), not any of these. The "Chromium-adjacent" consumers of this list are: **`lightningcss`** (Parcel/Svelte-adjacent tooling), **`swc_ecma_parser 46.1.2`** (Next.js/Deno-adjacent tooling), **`lol_html`** (Cloudflare edge), **`rustls`** (AWS/curl-adjacent, used by Linkerd/Firezone), **`resvg`** (SVG tooling ecosystem), **`wasmtime`** (Fastly/Shopify edge).

---

## 4. Q2 — licence compatibility for a permissive (MIT/Apache/BSD) project

**Clean (permissive, no obligations beyond attribution):** html5ever, markup5ever, tendril, taffy, harfrust, fontdb, swash, allsorts, cosmic-text, parley, skrifa/read-fonts, unicode-bidi, icu4x (Unicode-3.0), wgpu, the whole vello family, lyon, tiny-skia (BSD-3), resvg, objc2-metal (Zlib/Apache/MIT), metal, boa (Unicode-3.0/MIT), deno_core (MIT; V8 BSD-3), rquickjs, javascriptcore-rs, v8, wasmtime (**Apache-2.0 WITH LLVM-exception** — permissive, and the LLVM exception removes the usual Apache patent-termination worry for LLVM-derived code), wasmer, wasmi, hyper, h2, h3, quinn, rustls (Apache/ISC/MIT), reqwest, tokio, http-cache-semantics (BSD-2), cookie_store, base64, simd-json, lol_html (BSD-3), scraper (ISC).

**MPL-2.0 — the ones that need a decision:** `cssparser`, `selectors`, `stylo` + `stylo_dom`/`stylo_traits`/`stylo_static_prefs`, `style` (Firefox in-tree), `webrender` + `webrender_api`/`wr_glyph_rasterizer`, `mozjs`, `lightningcss`.

What MPL-2.0 actually means for a browser engine binary:

- It is **file-level** (weak) copyleft, not project-level. MPL files may be linked into a larger work under any licence, including proprietary or MIT.
- If you **modify** an MPL-covered file, you must make *that file's* source available under MPL-2.0. Distributing the binary requires telling recipients how to obtain the MPL-covered source.
- You may **not** relicense MPL code as MIT. Your `LICENSE` must remain "MIT/Apache-2.0 for our code, MPL-2.0 for these crates", and you must ship the MPL notices.
- **For this project (public, permissive, source-available) MPL-2.0 is compatible.** The obligations are: (a) publish your patches to MPL crates, (b) keep notices, (c) don't claim the whole binary is MIT. The costs are legal hygiene, not showstoppers.
- The real practical cost of MPL here is **fork maintenance**: Stylo's API churns every release (0.19 → 0.22 in 3 months), and every patch you carry must be published and re-applied.

**LGPL/AGPL/GPL: none.** I found no LGPL, AGPL or GPL crate in this candidate set. The two things people *mistake* for GPL problems:
1. **V8** is BSD-3-Clause, not GPL — but `deno_core`'s V8 static lib is a large binary you must redistribute with its notices.
2. **SpiderMonkey** is MPL-2.0 (not GPL) — but `mozjs` links prebuilt Firefox-derived binaries, so you are shipping Mozilla code and must comply with MPL for it.

**Honest summary:** a pure-MIT stack is achievable **only if you drop Stylo, `cssparser`, `selectors`, and WebRender** and write CSS yourself on top of a permissive tokeniser. There is no permissive, production-grade CSS cascade in Rust. That is the single biggest licensing-driven architecture decision in this document.

---

## 5. Q3 — dead ends (label these clearly)

| crate | last release | last commit | why it is a dead end |
|---|---|---|---|
| **`rustybuzz`** | 0.20.1 (2024-11-12) | 2026-07-26 (mirror noise) | **Explicitly archived by upstream.** README: "not developed further, unmaintained, and archived … switch to HarfRust". Migrate or you inherit HarfBuzz-10.1.0-era shaping bugs forever. |
| **`wasm3`** | 0.3.1 (2021-09-19) | 2022-03-04 | 61 months since release, 4.5 years since any commit. No SIMD, no threads, no reference types. |
| **`style`** (crates.io) | 0.1.0 (2019-12-21) | none (no repository) | Unrelated 2019 placeholder crate; 82 months stale. Not Servo's `style`. |
| **`lazy`** (crates.io) | 0.5.3 (2015-03-11) | 2015-03-24 | 139 months. Superseded by `std::sync::LazyLock` (stable since 1.80) and `once_cell`. |
| **`yoga` / `yoga-rs`** | 0.5.0 (2025-01-16) | 2026-10-12 (2 commits) | Near-dead: 21 months with no release, a 20-month commit gap, 163 stars, flexbox-only. Treat as unmaintained even though the repo is not archived. |

**Near-dead (watch, do not build on):** `piet` (superseded by Vello by the same org), `femtovg` (OpenGL-only on a Metal target), `h3` (17.6 months without a release), `wasm3` (above).

**Not dead but stale-release and worth watching:** `unicode-bidi` (22.3 mo release / active repo), `objc2-metal` (12.7 mo release / very active repo — the bindings are simply complete), `raw-window-handle` (29.3 mo, spec-stable, not in the requested list), `tiny-skia` (8.7 mo), `lyon` (7.6 mo but commits last week), `http-cache-semantics` (8.6 mo), `cookie_store` (8.2 mo, no commits either — the most at-risk "active" crate in the networking set).

---

## 6. Q4 — wgpu and Vello on Metal, in detail

### wgpu 30.0.1 on Metal (macOS arm64)

**Status: first-class, default-enabled, and production-proven.** The docs state: "It runs natively on Vulkan, **Metal**, D3D12, and OpenGL" and "It serves as the core of the WebGPU integration in **Firefox, Servo, and Deno**" ([docs.rs/wgpu](https://docs.rs/wgpu/latest/wgpu/)). The `metal` feature is on by default; MSRV is 1.87 (fine for edition 2024 on a current toolchain). Firefox pins a git revision of wgpu whose `naga` is 30.0.0, i.e. the same generation.

**What is genuinely good on Metal:** native bindless (buffer binding arrays are supported on Metal per the changelog), ray-tracing acceleration structures, `FLOAT32_BLENDABLE`, `SHADER_I16`, HDR/EDR surface color spaces with `Surface::display_hdr_info` (Apple EDR headroom is explicitly modelled via `DisplayHeadroom`), and `METALSharedEvent` interop for foreign-API handoff.

**Known caveats that will bite a browser compositor** (from the [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)):

1. **Alpha compositing semantics changed.** wgpu now advertises `CompositeAlphaMode::PreMultiplied` instead of `PostMultiplied` "matching the premultiplied alpha compositing that Core Animation actually performs for a non-opaque `CAMetalLayer`" (PR #9922, by Blitz's author). If you carry over post-multiplied assumptions you get haloed edges on translucent layers.
2. **Buffer-argument-table limits are tight and shared.** `Limits::max_buffers_and_acceleration_structures_per_shader_stage` was added because all buffer types share Metal's argument table; on Metal without `STRICT_WEBGPU_COMPLIANCE`, the combined limit **and** each individual per-type limit are pinned to **29**. A compositor that wants many uniform buffers + storage buffers per stage will hit this.
3. **Sampler arrays are capped lower than you'd think:** `max_binding_array_sampler_elements_per_shader_stage` was **reduced from 1024 to 996** on Apple7/Apple8/Mac2 to match Metal documentation (PR #10649).
4. **Indirect execution is the compute-renderer trap.** `DownlevelFlags::INDIRECT_EXECUTION` is only reported correctly for the iOS Simulator as of PR #10189, whose own text says "compute renderers such as vello previously failed with 'Downlevel flags DownlevelFlags(INDIRECT_EXECUTION) are required but not supported'". On real Apple Silicon this is fine; on the Simulator it is not.
5. **Version churn is the real cost.** v30.0.0 landed 2026-07-01, v30.0.1 2026-08-22, v29 was ~3 months earlier. Breaking changes every quarter; wire-format/API churn is guaranteed. Budget for it.

**Performance caveat (honest, partly unverified):** I could not find an authoritative, reproducible wgpu-vs-native-Metal benchmark for arm64 macOS in this session. Community claims range from "native Metal ≈ wgpu within ~15%" to "wgpu costs 20–40% on draw-call-heavy workloads" — treat both as **unverified**. What *is* verifiable is architectural: wgpu adds a validation + state-tracking + Naga-translation layer per draw and per pipeline creation. For a browser, pipeline creation cost is the known cliff (dozens of pipelines × shader variants on first paint), so use `PipelineCache` and warm pipelines off-thread. If you ever need to bypass wgpu, go to `objc2-metal` 0.3.2 (what wgpu-hal itself uses now), not the older `metal` crate.

### Vello on Metal — and how GPU text actually works

**Read the restructuring carefully, because it inverts the usual advice.** Upstream's [README](https://github.com/linebender/vello) defines three renderers:

| renderer | crate / version | architecture | upstream's own maturity statement |
|---|---|---|---|
| Vello CPU | `vello_cpu` 0.3.0 | Sparse Strips, CPU, SIMD + threads | "the most mature choice when predictable CPU rendering … is required"; "Vello CPU is currently overall more mature" |
| Vello GPU | `vello_gpu` 0.3.0 | Sparse Strips: CPU preprocessing + GPU raster/composite, **no compute shaders required**, wgpu (Metal by default) or WebGL2 | "intended to become the primary renderer for production GPU use cases **as it matures**" |
| compute renderer | `vello` 0.11.0 | GPU compute for almost everything | "remains an **experimental** implementation for compute-capable GPUs" |

**So the Metal-native answer is `vello_gpu` 0.3.0, not `vello` 0.11.0.** `vello_gpu` runs on wgpu's Metal backend with no compute requirement, which sidesteps the `INDIRECT_EXECUTION` class of problems entirely.

**Does Vello support GPU text, and how?** Yes, two different ways:

- **`vello_gpu`:** the `text` feature is **enabled by default** and exposes `Scene::glyph_run`. Glyphs are rasterised on the GPU from outlines, and with `full_image_support` (also default) you get **glyph atlas caching**, bicubic sampling, cropped external textures and alpha-mask tints. Text is therefore *GPU-rasterised glyph runs + cached atlas*, not SDF/MSDF and not CPU bitmap uploads per glyph.
- **`vello` 0.11 (compute):** `Scene::draw_glyphs(&FontData) -> DrawGlyphs` — a builder that encodes a glyph run; glyph outlines come from `skrifa 0.44` (a declared dependency), so shaping is yours (`harfrust`) and rasterisation is the GPU's.
- **`vello_cpu`:** text goes through **`glifo` 0.4.0** (the shared glyph-run crate under `glifo/` in the repo), CPU-side.

**What Vello does not give you:** no shaping (bring `harfrust`), no line breaking (bring `parley`/`icu`), no CSS `text-decoration` propagation, no subpixel-antialiasing parity with CoreText, no `backdrop-filter`. Vello is a *rasteriser with a scene API*, not a text stack and not a compositor.

**Compositing caveat:** nothing in the Vello family is a browser compositor. Servo pairs `vello`/`vello_cpu` with **WebRender 0.70.0** for the page and uses Vello for canvas; Blitz pairs `anyrender_vello` with its own shell. You will write the layer tree, damage tracking, and presentation yourself either way.

---

## 7. Q5 — mozjs vs deno_core for same-thread DOM + precise GC

This is the decision that most affects whether the engine can ever be *correct*, so it deserves the long answer.

### What "precise GC integration" actually requires
A DOM binding layer needs: (1) JS objects that own Rust DOM nodes with a **liveness edge visible to the collector**; (2) the ability to run **finalizers/destructors on the GC thread with a safe borrow of the DOM**; (3) **exact rooting** so a moving/compacting GC can update handles; (4) **same-thread, re-entrant** execution (JS calls `appendChild`, which mutates style/layout, which may run script again); (5) **interrupt/preemption** for long-running script; (6) **exact stack-limit and OOM behaviour**.

### mozjs 0.26.10 (SpiderMonkey 153.0)
- **This is the only option with a proven browser-shaped answer.** Servo *is* the proof: `mozjs 0.26.9` in Servo's lock, and the bindings are described as "battle-tested in Servo" ([README](https://github.com/servo/mozjs)). Servo's `script` crate implements exactly the DOM-binding, rooting (`RootedTraceable`, `JS::Rooted`), and finalization machinery you need — and it is MPL, so you can read and copy the pattern.
- **Same-thread DOM bindings:** yes, by design. SpiderMonkey is a same-thread engine with explicit `JSContext`, `JS::Handle`, `JS::MutableHandle`, `JS_EnterRealm`/`AutoRealm`, and a `JSTracer`/`TraceKind` protocol that lets you trace Rust-owned DOM pointers precisely. This is the "precise GC integration" the question asks about, and it exists here and essentially nowhere else in Rust.
- **Build complexity:** high but *managed*. SpiderMonkey is a large C++ build (Python + Clang/LLVM + `libclang` + `bindgen`). Crucially, `mozjs` now defaults to a **prebuilt archive** downloaded from `servo/mozjs` releases, with `MOZJS_CREATE_ARCHIVE`, `MOZJS_ARCHIVE=<path|https-url>`, and **`MOZJS_ATTESTATION`** (GitHub Attestations verification, `off`/`lenient`/`strict`) to confirm the archive was built in CI for a valid commit. `MOZJS_FROM_SOURCE=1` forces a source build. First-build worst case is tens of minutes to hours; steady state is a download.
- **Licence: MPL-2.0.** File-level copyleft. You link a SpiderMonkey binary/static lib built from Mozilla source; you must publish modifications to MPL files and keep notices, but your engine can stay MIT/Apache. This is compatible with a permissive open-source project.
- **Risk:** the bindings are a C++-shaped `unsafe` API. You will write a lot of `unsafe` in the binding layer, and SpiderMonkey's embedding API is not stable across ESR bumps (Servo re-syncs per ESR: currently `mozilla-esr153`).

### deno_core 0.413.0 (V8 152.2.0 via rusty_v8)
- **Spec compliance:** the best available (V8). If "runs real websites' JS" is the goal, V8 wins outright.
- **Same-thread DOM bindings:** possible but structurally awkward. `deno_core` is built as a *runtime*: `JsRuntime` owns an isolate, ops are registered as Rust functions marshalled across a `OpState`, and module loading/snapshotting is its own world. You can run it on a single thread (`!Send` futures + `LocalSet`), and you can create `v8::ObjectTemplate`/`FunctionTemplate` for DOM objects, but you are fighting the framework: `deno_core`'s op layer assumes thread-safe Rust (its `OpState`, `ResourceTable`, `JsRuntime::op` machinery), and its extension/snapshot system is designed for Deno's own architecture.
- **Precise GC integration:** the honest answer is **no, not in the sense Servo means it**. V8 exposes `v8::Weak<T>`, `v8::Global`, `v8::TracedReference`, `SetWeak` callbacks, and `EmbedderHeapTracer`-style APIs; that is enough for *conservative-ish* host-object lifetime management (which is what Node addons do), but it is not SpiderMonkey's `TraceTrait`-based precise tracing of Rust-owned DOM nodes. You will not get exact, moving-GC-safe rooting of DOM pointers without writing a V8 embedder-heap tracer and maintaining it against V8 API churn.
- **Build complexity:** deceptively easy. `cargo build` downloads a prebuilt static lib for `aarch64-apple-darwin` from `rusty_v8` releases by default ([README](https://github.com/denoland/rusty_v8)); `V8_FROM_SOURCE=1` compiles V8 (hours, needs `gn`/`ninja`/`clang`/Python). Cost: ~100 MB+ artifacts, long link times, and an API that breaks on every V8 roll (`deno_core` is at 0.413 — that is a *lot* of breaking releases).
- **Licence: MIT** crate; **V8 is BSD-3-Clause**. No copyleft problem at all.

### Verdict
- **For a rendering engine that intends to have a real DOM with precise GC and same-thread semantics: `mozjs`.** It is the only Rust binding with a production browser behind it, it gives you precise tracing of Rust-owned nodes, and Servo's MPL-licensed implementation is a blueprint you are allowed to study and adapt. Accept the C++ build, the prebuilt-archive plumbing, and MPL-2.0.
- **`deno_core` is the right choice only if you decide the DOM lives in JS.** If your architecture is "everything is JS objects, Rust is just the renderer", V8/`deno_core` is faster to a demo and far more spec-complete — but you will be rebuilding what Blink/WebKit do with their own binding generators, and you will not get precise tracing.
- **Do not plan on swapping later.** The binding layer is the hardest code in the engine; a JS-engine swap after 6 months is a rewrite. This is a one-way door and should be decided before the DOM object model is fixed.
- **Prototype tier:** `boa_engine` (pure Rust, 90% Test262, no JIT) is fine for CI/smoke tests and nothing else; `rquickjs` is fine for embedding user scripts; `javascriptcore-rs` is the fastest way to *see* real JS driving your DOM on macOS (system JSC, zero build) but its API is WKWebView-shaped and has no browser-grade GC protocol — treat it as a 2-week spike, not a foundation.

---

## 8. Recommended stack (if I had to commit today)

| layer | choice | why |
|---|---|---|
| HTML | `html5ever` 0.40 + `markup5ever` + `tendril` | Only real option; MIT/Apache; Servo+Blitz both use it |
| CSS parse | `cssparser` 0.38 + `selectors` 0.41 | Firefox+Servo shared; edition 2024; accept MPL |
| CSS cascade | **`stylo` 0.22 family** | Nothing else exists at this level; a six-figure LOC cascade you cannot write; MPL is fine for an open project |
| Layout | `taffy` 0.15 (flex/grid/block/float) + own inline/text flow | Proven in Servo's layout and Blitz; you must write inline formatting yourself |
| Text | `harfrust` + `skrifa`/`read-fonts` + `parley`(+`fontique`) + `unicode-bidi` + `icu_segmenter` | The exact combination Servo, Firefox and Blitz converge on |
| Fonts | `fontdb` (+ CoreText enumeration for system fonts) | Simple, MIT, Servo-proven |
| GPU | `wgpu` 30.0.1 (Metal default) | Firefox/Servo/Deno; the only sane Metal abstraction |
| 2D raster | `vello_gpu` 0.3 (+ `glifo`) with `vello_cpu` 0.3 as fallback/tests | Sparse Strips, no compute requirement, GPU glyph atlas |
| SVG | `resvg` 0.49 | Servo uses it |
| Compositing | yours (layer tree + damage) — optionally study `webrender` 0.70 | No drop-in exists |
| JS | `mozjs` 0.26 (SpiderMonkey 153) | Only precise-GC, same-thread, browser-proven option |
| Wasm | `wasmtime` 50 (or `wasmi` 2 if binary size matters) | Both Apache/MIT; wasm3 is dead |
| Networking | `hyper` 1.12 + `h2` + `quinn` + `rustls` 0.23 + `tokio` (IO threads only) + `http-cache-semantics` | Firefox ships hyper; Servo ships rustls; quinn is the best Rust QUIC |
| Data | `base64` 0.23; `serde_json` before `simd-json` | `simd-json`'s `unsafe`/mutable-input model is not worth it for untrusted page JSON |
| Window/input | `winit` 0.30 (0.31 is beta) + `raw-window-handle` 0.6 | wgpu-compatible; `winit 0.31.0-beta.4` (2026-10-24) not yet ready |

---

## 9. Top risk flags

1. **Stylo is a one-way door with MPL strings attached.** A six-figure LOC cascade you cannot replace, an API that moved 0.19 → 0.22 in three months, and file-level copyleft that forbids relicensing as MIT. If the project's licence story must be *purely* MIT/Apache, the entire CSS plan changes and the schedule multiplies. Decide this first, before any code.
2. **`rustybuzz` is archived and `vello` is now the experimental crate.** Two of the most commonly recommended crates in this space are traps. Correct names: `harfrust`, `vello_gpu`/`vello_cpu` (+ `glifo`).
3. **wgpu will break you quarterly.** v29→v30 landed 2026-07-01 with breaking changes, and Firefox refuses to track crates.io (pins a git rev). Budget ~1 week per wgpu major, forever, and keep the renderer behind your own thin abstraction.
4. **No permissive CSS cascade exists, and no engine-grade inline/table layout exists.** The two hardest subsystems (cascade + inline formatting) have no MIT/Apache drop-in. Everything else in this table is a solved problem by comparison.
5. **JS-engine choice cannot be deferred, and the two real options pull the architecture in opposite directions.** `mozjs` = Rust owns the DOM, precise tracing, MPL, heavy build. `deno_core` = JS owns the world, best spec compliance, MIT, awkward host-object GC. Choosing late means rewriting the binding layer, which is the most expensive code in the engine.

**Secondary risks:** `cookie_store` has had no commit since 2026-02-16 (you will likely need to fork it for partitioned cookies); `h3` has not released in 17.6 months while Firefox uses `neqo` instead; `unicode-bidi` is 22 months between releases; `objc2-metal` releases annually (bindings are complete, so this is acceptable); and the Metal buffer/sampler limits (29 buffers per stage, 996 samplers) are low enough that a naive per-layer bind-group design will hit them.

---

## 10. Explicitly unverified

- html5ever's current html5lib-tests pass rate (tests are in-tree; no published number I could fetch).
- taffy's grid/flex compliance percentage vs WPT (no published scorecard; Blitz's status page tests CSS *properties*, not taffy conformance directly).
- Any quantitative wgpu-vs-native-Metal performance ratio on arm64 macOS.
- Whether `blitz-dom` 0.3.0-beta.4's `stylo_taffy` bridge is API-stable enough to depend on directly (it is version-locked to Blitz releases).
- Vello's Speedometer-class performance on Apple Silicon under `vello_gpu` (no published benchmarks for the Sparse Strips GPU renderer yet).

---

## Appendix — raw verification data (2026-10-25)

`mo` = months since release; `commit` = latest commit date from the repo's `commits.atom` feed.

| crate | version | released | mo | licence | last commit |
|---|---|---|---|---|---|
| allsorts | 0.17.0 | 2026-05-13 | 5.4 | Apache-2.0 | 2026-05-13 |
| anyrender | 0.14.0 | 2026-10-03 | 0.7 | MIT OR Apache-2.0 | — |
| anyrender_vello | 0.15.0 | 2026-10-03 | 0.7 | MIT OR Apache-2.0 | — |
| anyrender_vello_cpu | 0.18.0 | 2026-10-03 | 0.7 | MIT OR Apache-2.0 | — |
| base64 | 0.23.1 | 2026-08-04 | 2.7 | MIT OR Apache-2.0 | 2026-08-18 |
| blitz | 0.3.0-beta.4 | 2026-10-16 | 0.3 | MIT OR Apache-2.0 | 2026-10-24 |
| blitz-dom | 0.3.0-beta.4 | 2026-10-16 | 0.3 | MIT OR Apache-2.0 | 2026-10-24 |
| boa_engine | 0.22.0 | 2026-08-28 | 1.9 | Unicode-3.0 OR MIT | 2026-10-17 |
| cookie_store | 0.22.1 | 2026-02-16 | 8.2 | MIT OR Apache-2.0 | 2026-02-16 |
| cosmic-text | 0.19.0 | 2026-04-22 | 6.1 | MIT OR Apache-2.0 | 2026-09-30 |
| cssparser | 0.38.0 | 2026-09-07 | 1.6 | MPL-2.0 | 2026-10-08 |
| deno_core | 0.413.0 | 2026-10-21 | 0.1 | MIT | 2026-10-25 |
| femtovg | 0.27.0 | 2026-08-31 | 1.8 | MIT OR Apache-2.0 | 2026-10-23 |
| fontdb | 0.24.0 | 2026-07-29 | 2.9 | MIT | 2026-07-29 |
| fontique | 0.12.0 | 2026-10-09 | 0.5 | Apache-2.0 OR MIT | — |
| glifo | 0.4.0 | 2026-10-02 | 0.8 | Apache-2.0 OR MIT | — |
| h2 | 0.4.20 | 2026-10-06 | 0.6 | MIT | 2026-10-09 |
| h3 | 0.0.8 | 2025-05-06 | 17.6 | MIT | 2026-09-20 |
| harfrust | 0.15.0 | 2026-10-22 | 0.1 | MIT | 2026-10-22 |
| html5ever | 0.40.1 | 2026-09-14 | 1.3 | MIT OR Apache-2.0 | 2026-09-14 |
| http-cache-semantics | 3.0.0 | 2026-02-04 | 8.6 | BSD-2-Clause | 2026-05-21 |
| hyper | 1.12.0 | 2026-10-06 | 0.6 | MIT | 2026-10-13 |
| icu | 2.4.0 | 2026-10-22 | 0.1 | Unicode-3.0 | 2026-10-23 |
| javascriptcore-rs | 2.0.0 | 2026-09-22 | 1.1 | MIT | 2026-09-22 |
| lazy | 0.5.3 | 2015-03-11 | 139.5 | MIT | 2015-03-24 |
| lightningcss | 1.0.0-alpha.72 | 2026-07-20 | 3.2 | MPL-2.0 | 2026-10-08 |
| lol_html | 3.0.2 | 2026-10-23 | 0.1 | BSD-3-Clause | 2026-10-23 |
| lyon | 1.0.19 | 2026-03-08 | 7.6 | MIT OR Apache-2.0 | 2026-10-23 |
| markup5ever | 0.40.0 | 2026-09-11 | 1.4 | MIT OR Apache-2.0 | 2026-09-14 |
| metal | 0.33.0 | 2025-12-17 | 10.2 | MIT OR Apache-2.0 | 2026-09-15 |
| mozjs | 0.26.10 | 2026-10-24 | 0.0 | MPL-2.0 | 2026-10-25 |
| objc2-metal | 0.3.2 | 2025-10-04 | 12.7 | Zlib OR Apache-2.0 OR MIT | 2026-10-17 |
| parley | 0.12.0 | 2026-10-09 | 0.5 | Apache-2.0 OR MIT | 2026-10-22 |
| piet | 0.8.0 | 2025-08-29 | 13.9 | Apache-2.0 OR MIT | 2026-05-02 |
| quinn | 0.11.12 | 2026-09-14 | 1.3 | MIT OR Apache-2.0 | 2026-10-23 |
| read-fonts | 0.46.0 | 2026-10-21 | 0.1 | MIT OR Apache-2.0 | 2026-10-23 |
| reqwest | 0.13.5 | 2026-09-08 | 1.5 | MIT OR Apache-2.0 | 2026-10-19 |
| resvg | 0.49.0 | 2026-10-24 | 0.0 | Apache-2.0 OR MIT | 2026-10-24 |
| rquickjs | 0.14.0 | 2026-09-18 | 1.2 | MIT | 2026-10-20 |
| rustls | 0.23.46 | 2026-10-14 | 0.4 | Apache-2.0 OR ISC OR MIT | 2026-10-23 |
| rustybuzz | 0.20.1 | 2024-11-12 | 23.4 | MIT | 2026-07-26 |
| scraper | 0.29.0 | 2026-10-21 | 0.1 | ISC | 2026-10-21 |
| selectors | 0.41.0 | 2026-09-30 | 0.8 | MPL-2.0 | 2026-10-22 |
| servo (umbrella) | 0.7.0 | 2026-10-05 | 0.7 | MPL-2.0 | 2026-10-25 |
| simd-json | 0.18.1 | 2026-08-23 | 2.1 | Apache-2.0 OR MIT | 2026-10-25 |
| skrifa | 0.49.0 | 2026-10-21 | 0.1 | MIT OR Apache-2.0 | 2026-10-23 |
| style | 0.1.0 | 2019-12-21 | 82.1 | MIT/Apache-2.0 | — |
| stylo | 0.22.0 | 2026-09-30 | 0.8 | MPL-2.0 | 2026-10-22 |
| stylo_dom | 0.22.0 | 2026-09-30 | 0.8 | MPL-2.0 | 2026-10-22 |
| stylo_static_prefs | 0.22.0 | 2026-09-30 | 0.8 | MPL-2.0 | 2026-10-22 |
| stylo_taffy | 0.2.0 | 2025-10-06 | 12.6 | MIT OR Apache-2.0 OR MPL-2.0 | 2026-10-24 |
| stylo_traits | 0.22.0 | 2026-09-30 | 0.8 | MPL-2.0 | 2026-10-22 |
| swash | 0.2.10 | 2026-07-17 | 3.3 | Apache-2.0 OR MIT | 2026-07-17 |
| swc_ecma_parser | 46.1.2 | 2026-10-21 | 0.1 | Apache-2.0 | 2026-10-22 |
| taffy | 0.15.0 | 2026-10-12 | 0.4 | MIT | 2026-10-21 |
| tendril | 0.5.1 | 2026-07-06 | 3.6 | MIT OR Apache-2.0 | 2026-09-14 |
| tiny-skia | 0.12.0 | 2026-02-02 | 8.7 | BSD-3-Clause | 2026-08-20 |
| tokio | 1.54.1 | 2026-10-21 | 0.1 | MIT | 2026-10-25 |
| unicode-bidi | 0.3.18 | 2024-12-16 | 22.3 | MIT OR Apache-2.0 | 2026-09-15 |
| unicode-segmentation | 1.13.3 | 2026-06-01 | 4.8 | MIT OR Apache-2.0 | 2026-10-18 |
| v8 | 152.2.0 | 2026-08-20 | 2.2 | MIT | 2026-10-21 |
| vello | 0.11.0 | 2026-10-02 | 0.8 | Apache-2.0 OR MIT | 2026-10-21 |
| vello_common | 0.3.0 | 2026-10-02 | 0.8 | Apache-2.0 OR MIT | 2026-10-21 |
| vello_cpu | 0.3.0 | 2026-10-02 | 0.8 | Apache-2.0 OR MIT | 2026-10-21 |
| vello_gpu | 0.3.0 | 2026-10-02 | 0.8 | Apache-2.0 OR MIT | 2026-10-21 |
| wasm3 | 0.3.1 | 2021-09-19 | 61.2 | MIT | 2022-03-04 |
| wasmer | 7.5.0 | 2026-10-01 | 0.8 | MIT | 2026-10-23 |
| wasmi | 2.0.0 | 2026-09-01 | 1.8 | MIT/Apache-2.0 | 2026-09-01 |
| wasmtime | 50.0.0 | 2026-10-20 | 0.2 | Apache-2.0 WITH LLVM-exception | 2026-10-23 |
| webrender | 0.70.0 | 2026-07-10 | 3.5 | MPL-2.0 | 2025-10-07 (repo feed) |
| wgpu | 30.0.1 | 2026-08-22 | 2.1 | MIT OR Apache-2.0 | 2026-10-25 |
| winit | 0.30.13 | 2026-03-02 | 7.8 | Apache-2.0 | 2026-10-24 |
| yoga | 0.5.0 | 2025-01-16 | 21.3 | MIT | 2026-10-12 |

### Sources
- crates.io API, per-crate: `https://crates.io/api/v1/crates/<name>` (versions, dates, licences, downloads) — queried 2026-10-25.
- Repo commit feeds: `https://github.com/<owner>/<repo>/commits.atom`.
- [wgpu docs (30.0.1)](https://docs.rs/wgpu/latest/wgpu/) · [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md) · [wgpu releases](https://github.com/gfx-rs/wgpu/releases)
- [vello README](https://github.com/linebender/vello) · [vello_gpu README](https://github.com/linebender/vello/tree/main/vello_gpu) · [vello releases](https://github.com/linebender/vello/releases) · [`Scene::draw_glyphs`](https://docs.rs/vello/0.11.0/vello/struct.Scene.html#method.draw_glyphs)
- [rustybuzz README (archived notice)](https://github.com/harfbuzz/rustybuzz) · [harfrust README](https://github.com/harfbuzz/harfrust)
- [servo/mozjs README](https://github.com/servo/mozjs) · [rusty_v8 README](https://github.com/denoland/rusty_v8) · [boa README](https://github.com/boa-dev/boa)
- [Servo Cargo.lock](https://github.com/servo/servo/blob/main/Cargo.lock) · [Firefox Cargo.lock](https://github.com/mozilla-firefox/firefox/blob/main/Cargo.lock) · [Ladybird Cargo.toml](https://github.com/LadybirdBrowser/ladybird/blob/master/Cargo.toml)
- [Blitz README](https://github.com/DioxusLabs/blitz) · [Blitz CSS status](https://blitz.is/status/css) · blitz-dom dependencies via crates.io API
- [Bug 2070964 — Upstream Stylo changes from Servo (Aug 2026)](https://bugzilla.mozilla.org/show_bug.cgi?id=2070964)
- [html5ever workspace Cargo.toml](https://github.com/servo/html5ever/blob/main/Cargo.toml) · [cssparser Cargo.toml](https://github.com/servo/rust-cssparser/blob/main/Cargo.toml)
- [bschwind/yoga-rs](https://github.com/bschwind/yoga-rs)
