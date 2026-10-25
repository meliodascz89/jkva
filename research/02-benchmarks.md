# 02 — How a Browser Engine Is Measured, and How You Would Prove You Beat the Best

**Date:** 2026-10-25 (Europe/Prague)
**Target:** a new Rust browser engine for Apple Silicon (arm64 macOS, Metal)
**Question:** what does the industry actually measure, what are the real numbers on Apple Silicon today, what does conformance look like in mechanics (not vibes), how do you measure memory and energy on macOS without fooling yourself — and what can a v0.1 engine honestly claim in 12 months?

**Verification legend used throughout:**
- **[V]** — verified in this session from a primary source (vendor blog, project repo, man page, or a measurement I ran on this machine).
- **[V-2nd]** — verified from a credible secondary source that cites a primary I could not fetch (paywall, redirect block, LinkedIn).
- **[C]** — computed by me from primary artifacts. Method stated inline; reproducible.
- **[U]** — **unverified / could not confirm.** Do not build a claim on it.

Local machine used for first-party checks: **Mac mini (Mac16,10), Apple M4, 10 cores (4P+6E), 24 GB, macOS 27.0.1 (26A434)**.

---

## 0. Bottom line up front

1. **The benchmark suite turned over in 2026.** JetStream 3.0 replaced a seven-year-old JetStream 2 on 2026-03-31; Speedometer is at 3.1; MotionMark at 1.3.2; WebXPRT at 5. Octane and Kraken are effectively gone as standalone suites (Google's Octane URL now 301-redirects to JetStream 3 **[V]**), and their workloads live on *inside* JetStream 3. Any measurement plan written before mid-2026 is quoting dead numbers.
2. **JetStream 3 scores are not comparable to JetStream 2 scores**, and the announcement says so explicitly **[V]**. Mixing them is the single most common error in 2026 browser-comparison writing.
3. **Speedometer is not a headless-friendly benchmark in the naive sense.** It needs a real 850×650+ viewport, iframes, `contentWindow` navigation, Web Components/shadow DOM, contenteditable editors, canvas, and SVG — and its harness approximates async work with a zero-second timer **[V]**. A "minimal shell" must be a real browser shell, not a script host.
4. **Conformance is now a hard gate, not a scoreboard.** Apple's EU alternative-browser-engine entitlement requires **90% of WPT** and **80% of test262**, *and requires the same result with JIT disabled* **[V]**. That is a procurement bar a new engine must eventually clear to ship on iOS at all.
5. **WPT pass rates on 2026-10-25 (computed by me from wpt.fyi summary artifacts):** Chrome 156 stable **97.1%**, Firefox 158 stable **95.6%**, Safari 27.0 **94.2%**, Servo 0.7.0 **90.4%** on their own denominators; aligned to a common denominator, **97.1 / 94.5 / 93.5 / 88.3** **[C]**. This is the honest yardstick for "engine completeness", and it is brutal.
6. **`powermetrics` is explicitly not a cross-device comparison tool.** Apple's own man page says average power values "are estimated and may be inaccurate — hence they should not be used for any comparison between devices" **[V]**. Activity Monitor "Energy Impact" is an undocumented composite; Mozilla's own docs say it "should be avoided" **[V]**.
7. **The most defensible "we beat an incumbent" claim available to a Rust engine is a JS-engine-matched one.** If you embed SpiderMonkey and compare your engine+SpiderMonkey against Firefox+SpiderMonkey on the same machine, you isolate exactly the browser-engine delta. Everything else is confounded **[V]** for the reasoning; **[C]** for the recommendation.

---

## 1. User-facing browser benchmarks

### 1.1 Master table (state on 2026-10-25)

| Benchmark | Current version | What it actually measures | Automated/headless? | Needs a full browser? | Time per run |
|---|---|---|---|---|---|
| **Speedometer** | **3.1** (2025-03-31) **[V]** | Responsiveness of *web applications*: 32 workloads of timed synthetic user interaction across TodoMVC frameworks, editors, charts, news sites **[V]** | Yes, via URL params + WebDriver/Selenium (`crossbench`) **[V]** | **Yes** — needs layout, paint, iframes, ≥850×650 viewport | ~2–5 min |
| **JetStream** | **3.0** (2026-03-31) **[V]** | JS + Wasm throughput on 77 larger, more realistic workloads; geometric mean of startup/worst/average **[V]** | Yes; **designed to run in engine shells** **[V]** | **No** — shells OK | ~10–20 min |
| **MotionMark** | **1.3.2** **[V]** | Graphics: 8 subtests (CSS, SVG, Canvas2D) ramping element count until frame rate collapses; score = change point **[V]** | Partly — needs real compositor + `requestAnimationFrame` cadence | **Yes** | ~5–10 min |
| **WebXPRT** | **5** (+ optional AI Image Generation workload, 2026-09-23) **[V]** | 7 scripted "daily task" scenarios incl. 4 AI/ML ones (face detect, image classify, doc scan, video background blur), plus photo effects, science project, spell check **[V]** | **No documented path** **[U]** | **Yes** — interactive, runs "right from your browser" | ~5–15 min |
| **ARES-6** | *retired* | JS engine microbenchmarks | n/a | n/a | — |
| **Basemark Web** | **3.0**, now served from `web.gpuscore.com` **[V]** | Mixed "real-world" web workload incl. graphics | Unclear **[U]** | **Yes** | — |
| **Kraken** | **dead** — `browserbench.org/Kraken/` → 404; `krakenbenchmark.mozilla.org` no longer resolves **[V]** | — | — | — | — |
| **Octane 2** | **retired** — `developers.google.com/octane` → 301 → `browserbench.org/JetStream/` (JetStream 3.0) **[V]** | — | — | — | — |
| **Procyon Essentials** *(2026 newcomer)* | 1.0 (2026-05-05) **[V]** | Browser-focused multitasking: video call, **up to 30 tabs**, chat with AI summarisation, file ops **[V]** | No — commercial harness | Yes, but **Windows PCs only** **[V]** | long |

### 1.2 Per-benchmark detail

#### Speedometer 3.1 — the responsiveness benchmark
- **Release:** 3.1 on **2025-03-31**, a bug-fix release over 3.0 (2024-03). Fixes: Next.js/Nuxt workload doing unequal work across browsers, constant ID regeneration in lists, and a synchronous task in the perf-dashboard workload that wasn't actually being measured **[V]**.
- **Governance:** Apple + Google + Microsoft + Mozilla under an open governance model, shared repo **[V]**.
- **Workloads: 32** in `resources/tests.mjs` **[V]**, in four journeys **[V]**:
  - *Todo list:* 12 frameworks × base/complex = 24 suites — vanilla ES5, ES6+webpack, Web Components, React, React-Redux, Backbone, Angular, Vue, jQuery, Preact, Svelte, Lit.
  - *Rich text editing:* CodeMirror, TipTap.
  - *Charts:* observable-plot, Chart.js, React-Stockcharts (SVG), WebKit perf dashboard.
  - *News site:* Next.js, Nuxt.
- **Automation:** URL parameters `startAutomatically`, `iterationCount`, `suite`/`suites`, `tags`, `viewport`, `warmupBeforeSync`, `measurementMethod` (`timer`|`raf`), `shuffleSeed`, `developerMode`, `generate` **[V]**. Results are exposed as JSON (`Copy JSON` / `Download JSON`) **[V]**.
- **Canonical runner:** Chromium's **`crossbench`** — Selenium-based, supports Chrome/Chromium, Firefox, Safari, Edge; flags `--repeat=`, `--headless`, `--stories='regex'`, `--network=/path/to/speedometer` (local file server for determinism), `--probe=profiling` **[V]**.
- **Methodology you must not misquote:** it synchronously executes many operations (e.g. 100 todo items) to get measurable durations; it approximates async UI work with a *zero-second timer* scheduled after each sync block; it **does not measure work in Web Workers**; and it must not be used to compare JS frameworks **[V]**.
- **A good score:** desktop Apple Silicon (M2-class machine) 2026: **41.9–42.7**; M4 MacBook Pro class: **~52**; M5 MacBook Pro: **61** **[V-2nd]**. Treat ≥40 as "competitive", ≥55 as "class-leading".

#### JetStream 3.0 — the throughput benchmark
- **Release:** **2026-03-31**, first major version since JetStream 2 (2019); 200+ PRs, 500+ files, 70+ workloads over 1.5 years **[V]**.
- **77 default workloads** **[V]**, plus disabled-by-default ones (e.g. `Dart-flute-complex-wasm`).
- **Scoring:** each workload runs N iterations (usually 120); startup = first iteration, worst case = mean of worst M (usually 4) excluding first, average case = mean of all but first; the three are geometrically averaged; workload scores are geometrically averaged into the total **[V]**. `WSL` is scored differently (compile time + spec-suite runtime) **[V]**.
- **What changed vs JetStream 2 (and why scores moved):** Wasm scoring now includes compilation and instantiation in the first iteration instead of a separate Startup/Runtime split — JetStream 2.2 had to *clamp* scores at 5000 because startup times hit 0 ms and the score formula `5000/Time` produced `Infinity` **[V]**. Fewer microbenchmarks, more large applications; asm.js removed; new coverage of Promises/async, private/public class fields, modern RegExp, BigInt, WasmGC, Wasm SIMD, Wasm exception handling; toolchains include C++, C#, Dart, Java, Kotlin, Rust **[V]**.
- **Explicit warning in the docs:** "scores from JetStream 3 are not comparable to scores to other versions of any JetStream benchmark" **[V]**.
- **Automation:** runs in engine shells, explicitly recommended for device emulators and constrained systems **[V]**. `?test=` URL parameter selects workloads **[V]**. `crossbench` has `jetstream_2_2.py`; check the current benchmark id before scripting **[U]** (I did not confirm a `jetstream_3` crossbench module).
- **A good score:** M4 MacBook Air, Safari 26.0 → **359**, Safari 26.4 → **395** **[V]**. M5 MacBook Pro, Chrome → **469** **[V-2nd]**. iPhone 17 Pro Max: Safari **270.9**, Blink prototype **306.35** **[V-2nd]**.

#### MotionMark 1.3.2 — the graphics benchmark
- **8 core subtests:** Multiply (CSS border-radius/transforms/opacity), Canvas Arcs, Leaves (CSS-transformed elements), Canvas Paths, Canvas Lines, Images (`getImageData`/`putImageData`), Design (HTML text rendering), Suits (SVG clip paths/gradients/transforms) **[V]**.
- **Method:** warm-up, then a fixed time budget per test; the number of elements is adjusted to converge on the point where the browser stops hitting target frame rate; piecewise linear regression finds the change point; **the confidence interval is computed by bootstrapping**; total = geometric mean of subtest scores **[V]**.
- **Critical harness constraint:** it depends on `requestAnimationFrame` cadence, and **Safari fires rAF at 60 Hz on 120 Hz panels while other browsers fire at 120 Hz** — so cross-browser comparison *requires* setting the display to 60 Hz **[V]**. Ignoring this is the #1 way to publish a wrong MotionMark comparison.
- **Viewport classes:** small 568×320, medium 900×600, large 1600×800 **[V]**. Score depends on the class, so always report it.
- **Version note:** Magic Lasso's January 2026 numbers are on **1.3.1**; 1.3.2 changed the rubric wording ("fullscreen" → "maximize") **[V]**. 1.3.1 increased the max complexity of Multiply and made subtest scores cap at the maximum when complexity saturates **[V]** — i.e. 1.3.1+ scores are not comparable to 1.3.0.
- **A good score:** M2 MacBook Air 15", Safari 26.2 → **8088.73**; Chrome 143 → **6582.95**; Firefox 146 → **1497.44** **[V-2nd]**. ≥5000 = good; ≥8000 = class-leading on a laptop-class Apple SoC.

#### WebXPRT 5
- 7 main scenarios, four of them AI/ML in-browser workloads, plus an **optional experimental AI Image Generation** workload that runs separately and does not change the main score **[V]**. WebXPRT 4 remains available **[V]**. There is a public results database **[V]**.
- **Automation status [U]:** I found no documented headless/CLI automation path. Treat WebXPRT as a manual, browser-UI, human-supervised benchmark. That alone makes it near-useless for CI.

#### Retired suites — what to do with them
- **Octane 2:** Google redirects the Octane page to JetStream 3 **[V]**. Its workloads (`Box2D`, `crypto`, `deltablue`, `earley-boyer`, `gbemu`, `mandreel`, `navier-stokes`, `pdfjs`, `raytrace`, `regexp`, `richards`, `splay`, `typescript`) survive as JetStream 3 line items **[V]**. **Do not report Octane scores in 2026.**
- **Kraken:** the hosted benchmark is gone (404 / DNS failure) **[V]**. Kraken-inspired workloads (`ai-astar`, `gaussian-blur`, `stanford-crypto-*`, `json-parse-inspector`, `json-stringify-inspector`) live in JetStream 3 **[V]**.
- **ARES-6:** superseded; `Air`, `Basic`, `Babylon`, `ML` are JetStream 3 workloads **[V]**. `browserbench.org/ARES-6/` still serves a page, which is a trap for anyone citing "current" numbers from it **[V]**.
- **SunSpider:** explicitly de-emphasised in JetStream 3 ("lowered their importance") **[V]**.

#### 2025–2026 newcomers, assessed
| Newcomer | Date | Verdict for this project |
|---|---|---|
| Speedometer 3.1 | 2025-03-31 | **Adopt.** The reference responsiveness metric. |
| JetStream 3.0 | 2026-03-31 | **Adopt** as the JS/Wasm companion; only suite that runs in a shell, so it is your first automatable signal. |
| MotionMark 1.3.2 | 2026 | **Adopt with care** — needs compositor, 60 Hz, fixed viewport, and is the least achievable win. |
| WebXPRT 5 | 2026 | **Watch.** Real-world-ish and AI-loaded, but manual and Windows/mobile-leaning in its published corpus. |
| Procyon Essentials | 2026-05-05 | **Reject for now** — explicitly "broad compatibility with Windows PCs" **[V]**; also 30-tab multitasking is a memory/process-model test more than an engine test. |
| Interop 2026 dashboard / webstatus.dev / Edge "Top Developer Needs" | 2026-02-12 etc. | **Adopt as conformance reporting**, not performance. |

### 1.3 Minimum web-platform surface

This is the part most "we'll just run it headless" plans get wrong. Derived from the actual harness source and workload list **[V]**.

**Speedometer 3.1 harness API inventory** (grepped from `resources/benchmark-runner.mjs` + `main.mjs`):
- ES modules (`.mjs`, `import`), `async`/`await`, `Promise`
- `<iframe>` creation + `contentWindow` + **navigation of the iframe** (`window.location`) — the runner loads each workload into a fresh iframe per iteration
- **User Timing**: `performance.mark`, `performance.measure`, `performance.now`
- `setTimeout` (including the zero-second async approximation), `requestAnimationFrame`
- **Layout-dependent**: `window.innerWidth`/`innerHeight`, `document.elementFromPoint`
- DOM core: `getElementById`, `createElement`, `querySelector(All)`, `document.body`, `documentElement`, `title`, `createEvent`, `addEventListener`, `history`
- **Shadow DOM**: `shadowRoot` (needed by the Web Components workloads)

**Workload-level requirements beyond the harness:**
| Workload family | Additional surface |
|---|---|
| TodoMVC ×12 frameworks | ES5 and ES2015+ (classes, modules, spread, Proxy for Vue 3), custom elements + shadow DOM (Web Components, Lit), CSS layout/paint, form controls, keyboard events (`keyup`, `keypress`, `change`), `localStorage`-free static build |
| CodeMirror / TipTap | `contenteditable`, Selection/Range APIs, beforeinput/input events, `execCommand`-adjacent editing, `getClientRects`/`getBoundingClientRect`, scroll metrics |
| Charts | SVG (paths, transforms, clip), `<canvas>` 2D (paths, arcs, gradients, text), `getImageData`/`putImageData` |
| News sites (Next.js, Nuxt) | hydration over a large static DOM, `<script type="module">`, CSS custom properties, responsive layout |
| perf.webkit.org dashboard | deep DOM trees, tables, SVG charts, heavy style recalculation |

**Practical conclusion:** the minimum viable shell for Speedometer is *DOM + CSS cascade + layout + paint + compositing + events + shadow DOM + canvas 2D + SVG + contenteditable*. That is most of a browser. There is no cheap subset.

**JetStream 3.0** is the opposite: it needs only a JS + Wasm execution environment. Decompose it into two questions: (a) *your JS engine* — run JetStream in the shell, no DOM at all; (b) *your full engine* — run it in-browser to capture DOM-bound line items (`web-ssr`, `mobx-startup`, `proxy-vue`, `threejs`). **Both numbers are legitimate, but they answer different questions and must be labelled.**

**MotionMark** needs: CSS transforms/opacity/border-radius, SVG clip paths and gradients, Canvas 2D paths and `getImageData`, HTML text rendering, and a real frame scheduler with vsync-aligned `requestAnimationFrame`. It is the benchmark most hostile to a fresh engine, because a naive software rasterizer will lose by 5–10×.

---

## 2. Real reference numbers on Apple Silicon

### 2.1 The numbers, with provenance

| Machine | OS | Browser | Speedometer 3.1 | JetStream | MotionMark | Source, date |
|---|---|---|---|---|---|---|
| **M2 MacBook Air 15", 16 GB** | macOS 26.2 (Tahoe) | **Safari 26.2** (21623.1.14.11.9) | **41.9** | 436.8 *(JS **2.2**)* | **8088.73** (1.3.1) | Magic Lasso, tested 2026-01-08 **[V-2nd]** |
| | | **Chrome 143.0.7499.193** | **42.7** | 419.2 *(2.2)* | **6582.95** (1.3.1) | same **[V-2nd]** |
| | | Edge 143.0.3650.96 | 40.8 | 413.3 *(2.2)* | 6003.25 (1.3.1) | same **[V-2nd]** |
| | | **Firefox 146.0.1** | **35.7** | 270.9 *(2.2)* | **1497.44** (1.3.1) | same **[V-2nd]** |
| **M4 MacBook Air** | macOS 26.0 | Safari 26.0 | — | **359** *(JS **3**)* | — | WebKit blog, 2026-03-31 **[V]** |
| **M4 MacBook Air** | macOS 26.4 | Safari 26.4 | — | **395** *(JS 3)* | — | WebKit blog, 2026-03-31 **[V]** |
| **M4 MacBook Pro** | macOS 15 | Chrome 139 | **52.35** *(version disputed — see §2.3)* | — | — | Chromium blog + MacRumors, 2025-06-05 **[V-2nd]** |
| **M5 MacBook Pro** | macOS 26.0.1 | Chrome | **61** | **469** *(JS 3)* | — | Google blog / MacRumors, 2026-06-05 **[V-2nd]** |
| **iPhone 17 Pro Max** | iOS 26.5.1 | Safari 26.5.1 | 38.3 | 270.9 *(JS 3)* | 4673.68 (1.3.1) | OWA, 2026-06-18 **[V-2nd]** |
| | | Blink prototype (Chromium 151) | **49.27 (+28.6%)** | 306.35 (+13.1%) | 4773.52 (+2.1%) | same **[V-2nd]** |

Cross-checks I ran on the same day against wpt.fyi's run inventory, to pin "current stable" **[V]**:
Chrome **156.0.8078.47**, Firefox **158.0.1**, Safari **27.0 (22625.1.29.11.27)**, Servo **0.7.0**, Chromium nightly **158.0.8122.0**.

> ⚠️ **M4 + Safari + Speedometer 3.1 is not in any source I could verify.** Apple has not published a Speedometer 3.1 figure for Safari on M4; MacRumors noted in June 2025 that Apple "has not recently shared its maximum Speedometer 3 test results" **[V-2nd]**. **Do not interpolate one.** If you need that number, measure it — the machine in this workspace is an M4.

### 2.2 What is CPU-bound vs GPU-bound

| Benchmark | Bound by | Evidence |
|---|---|---|
| **Speedometer 3.1** | **CPU** — JS execution, DOM mutation, style/layout invalidation. Pixel work is real but small. | Harness measures wall time around synchronous DOM/JS batches; explicitly excludes worker concurrency **[V]** |
| **JetStream 3.0** | **CPU** — pure JS/Wasm, no rendering at all. | Runs in engine shells with no DOM **[V]** |
| **MotionMark 1.3.2** | **GPU-raster + compositing**, not shader compute | All 8 subtests are CSS/SVG/Canvas2D **[V]**; no WebGL/WebGPU subtest in the core suite |
| **WebXPRT 5** | Mixed; the 4 AI scenarios are **CPU SIMD/Wasm** heavy (with GPU only if the engine's ML path uses it) | Scenario names and Wasm usage **[V]** |
| **Real-world video playback** | **Fixed-function video decode + display pipeline** — the engine barely matters if you use `AVSampleBufferDisplayLayer`/VideoToolbox correctly | Reasoned from the platform; not benchmarked here **[U]** |

The strategic consequence: **MotionMark is the one benchmark where a new engine's *architecture* (Metal-native compositor vs software raster) determines the answer, and where the incumbent's lead is largest.** Speedometer/JetStream are contests of JS+DOM engineering that you can lose gracefully; MotionMark is a contest you will lose badly for years.

### 2.3 Version-comparability traps (read before quoting anything)

1. **JetStream 2.x ≠ JetStream 3.x.** The vendor documentation says so outright **[V]**. Example of the confusion: Firefox scores **270.9 on JetStream 2.2** (M2, 2026-01) and Safari scores **270.9 on JetStream 3** (iPhone, 2026-06) **[V-2nd]**. Identical numbers, different benchmarks, pure coincidence. A 2026 article that ranks Firefox using JetStream 2.2 numbers next to Chrome's JetStream 3 numbers is measuring nothing.
2. **Chrome's "52.35" record is ambiguous.** Google's June 2025 post is titled Speedometer 3.1; MacRumors, reporting the same number, wrote "it is worth noting that Google appears to have used Speedometer 3, and not the newer Speedometer 3.1 test" **[V-2nd]**. The secondary literature then propagated it as 3.1. Treat 52.35 as "Speedometer 3.x on M4, mid-2025" and say so.
3. **MotionMark 1.3.0 → 1.3.1 changed the ceiling**, so a 1.3.1+ score can exceed what 1.3.0 could produce **[V]**.
4. **`requestAnimationFrame` cadence** silently changes MotionMark by ~2× if the panel is 120 Hz and one browser fires rAF at 60 Hz **[V]**. Set the display to 60 Hz.
5. **Safari ships with the OS.** A "Safari 26.2" number is a macOS 26.2 number. Chromium versions are independent. Comparing "latest Safari" to "latest Chrome" across different macOS point releases is comparing two variables.

---

## 3. Engine-level conformance

### 3.1 WPT: the mechanics

**What a run produces.** `wpt run <product> <paths>` drives a browser over WebDriver and emits, per test, a status (`PASS`/`FAIL`/`ERROR`/`TIMEOUT`/`CRASH`/`SKIP`/`NOTRUN`) plus per-subtest results. Machine-readable output: `--log-wptreport=report.json` (+ `--log-wptscreenshot=` for reftests); that pair is exactly what wpt.fyi ingests **[V]**.

**Test types — and they are not interchangeable** **[V]**:
| Type | Mechanism | What a new engine needs |
|---|---|---|
| **testharness.js** | JS asserts expected IDL/JS behaviour, reports via harness | Full JS + IDL bindings; no pixels involved → the *easiest* early target |
| **reftest** | Renders test and reference; must match pixel-for-pixel within `fuzzy` tolerance | A renderer, plus a screenshot path and a comparison tool |
| **visual test** | Human looks, or compares against a saved per-UA screenshot | n/a for CI |
| **wdspec** | Python tests of the WebDriver protocol itself | A WebDriver server |
| **aamtest** | Accessibility-tree mapping (Core-AAM / HTML-AAM) | An accessibility tree |
| **manual** | Human | Excluded from automation |

**Running against a non-Blink/Gecko engine — the actual prerequisites:**
1. **A WebDriver implementation.** `wpt run` finds products through `wptrunner`'s per-browser modules (`wptrunner/browsers/*.py`) and drives them over WebDriver **[V]**. There is no "point it at an arbitrary binary" mode. For a new engine you either (a) implement a WebDriver HTTP server in your shell, (b) adopt an existing WebDriver that speaks to your embedding (e.g. WebKitGTK MiniBrowser / WPE headless are documented first-class products **[V]**), or (c) write a `wptrunner` product module.
2. **Hosts-file + TLS.** `./wpt make-hosts-file | sudo tee -a /etc/hosts`, then `./wpt serve` (HTTP :8000, HTTPS :8443, WebSocket) **[V]**. WPT's own root CA must be trusted for the `https` origins **[V]**.
3. **A manifest, used as canon.** Enumerate tests with `./wpt manifest`, not by walking directories — the docs call its behaviour "the canonical definition of how to enumerate tests and find their type" **[V]**.
4. **Expectation metadata (`.ini`)** — see §3.2.

**Reporting pass rates honestly.** wpt.fyi's headline is subtests-passed / subtests-run. Two different denominators exist and they give different answers:
- **Own denominator:** passes ÷ the subtests *that engine ran*. Rewards engines that skip hard tests.
- **Aligned denominator:** passes ÷ the maximum subtests *any* engine ran for that test. This is what Apple's entitlement criterion uses — "as a percentage of the highest number of subtests executed by any browser on the wpt.fyi front page" **[V]**.
  Always state which one you used. The gap is small for mature engines (Chromium: 97.13 → 97.05) and large for incomplete ones (Servo: 90.44 → 88.30) **[C]**.

### 3.2 The `-expected` metadata you need

Metadata lives under a `metadata/` root mirroring the test tree, with `.ini` appended: test `/spec/section/file.html` → `metadata/spec/section/file.html.ini` **[V]**. Files are ini-like but with **hierarchical, whitespace-significant sections** and **`:` as the only key/value separator** **[V]**.

Keys that matter for a new engine **[V]**:
| Key | Use |
|---|---|
| `expected:` | `PASS`/`FAIL`/`ERROR`/`TIMEOUT`/`CRASH`/`SKIP`; a **list** means "primary + known-intermittent", e.g. `expected: [PASS, ERROR]` |
| `disabled:` | any value except `@False` disables the test/subtest (put a bug URL there) |
| `restart-after:` | force a browser restart — essential if your engine leaks state between tests |
| `fuzzy:` | reftest pixel tolerance: `[maxPerChannel; maxTotalPixels]`, optionally per-reference or per-pair |
| `implementation-status:` | `implementing` / `not-implementing` / `backlog`; pair with `--skip-implementation-status` |
| `tags:` | for `--tag` selection |
| `__dir__.ini` | directory-wide defaults |
| conditional values | `if os == "mac": FAIL` — Python-ish expressions over `RunInfo` (`product`, `browser_channel`, `debug`, `os`, `version`, `processor`) |

Generate the baseline instead of hand-writing it: `./wpt update-expectations --full <wptreport.json…>`, with a `--properties-file` declaring which conditionals to use (default `metadata/update_properties.json`) **[V]**.

**Honest use of this mechanism:** `expected: FAIL` on a test you haven't implemented is bookkeeping, not conformance. Publish two numbers — *"pass rate over the tests we claim to implement"* and *"pass rate over the full suite"* — and always the latter with the aligned denominator if you want to be comparable to Safari/Chrome.

### 3.3 Measured WPT pass rates, 2026-10-25 **[C]**

Because no official headline was fetchable for all four engines on the same day, I computed this from the wpt.fyi **summary_v2** artifacts (format: `{test_path: {"s": status_char, "c": [passes, total_subtests]}}`):

```bash
# Reproduce. Run ids from https://wpt.fyi/api/runs?product=<name>&max-count=1&labels=stable
curl -s "https://wpt.fyi/api/runs?product=safari&max-count=1&labels=stable" | jq -r '.[0].results_url'
# download the .json.gz, gunzip, then:
python3 - <<'EOF'
import json
D = {e: json.load(open(f"s_{e}.json")) for e in ("chrome","firefox","safari","servo")}
for e, d in D.items():
    p = sum(v["c"][0] for v in d.values() if v["s"] != "S")
    t = sum(v["c"][1] for v in d.values() if v["s"] != "S")
    print(f"{e:8s} {p:9,d}/{t:9,d} = {100*p/t:6.2f}%")   # own denominator
EOF
```

Results:

| Engine (run) | Tests | Subtests passed / run | Pass % (own denom.) | Pass % (aligned) |
|---|---|---|---|---|
| Chrome **156.0.8078.47** (linux 22.04) | 125,334 | 2,198,244 / 2,263,239 | **97.13%** | **97.05%** |
| Firefox **158.0.1** (linux 22.04) | 125,334 | 2,140,506 / 2,239,507 | **95.58%** | **94.50%** |
| Safari **27.0** (mac 27.0) | 124,704 | 2,118,045 / 2,247,498 | **94.24%** | **93.51%** |
| Servo **0.7.0** (linux 22.04) | 124,702 | 1,999,992 / 2,211,369 | **90.44%** | **88.30%** |

Aligned denominator = 2,265,070 subtests.

**Per-directory pass % (own denominator)** — this is where a new engine's roadmap actually lives:

| Directory | Chrome | Firefox | Safari | Servo |
|---|---|---|---|---|
| `/css/` | **93.4** | 85.1 | 87.6 | **55.4** |
| `/html/` | 96.7 | 94.5 | 93.8 | 82.7 |
| `/dom/` | 98.6 | 97.9 | 97.7 | 90.1 |
| `/fetch/` | 94.1 | 90.7 | 86.9 | 85.0 |
| `/webidl/` | 93.8 | 96.4 | 94.2 | 95.1 |
| `/url/` | 94.3 | 89.4 | **99.9** | 90.0 |
| `/svg/` | 96.1 | 97.6 | 92.0 | **37.7** |
| `/fullscreen/` | 89.9 | 90.3 | 73.0 | 71.8 |
| `/webdriver/` | 95.4 | 90.9 | **7.0** | 32.6 |

**Caveats, stated plainly:** (a) this is my computation, not a wpt.fyi headline — wpt.fyi's SPA endpoints did not yield JSON scores in this session; (b) the Safari run is on a *different WPT revision* (`f4febe30…`) than the Chromium/Firefox/Servo runs (`5af3c135…`), so it is same-day but not same-commit; (c) `SKIP` is excluded and therefore each engine's denominator reflects what it chose to run; (d) status characters observed were `O`(OK), `P`(PASS), `F`, `E`(ERROR), `T`(TIMEOUT), `S`(SKIP), `C`(CRASH).

**Independent corroboration:** Alex Russell's July 2026 analysis reports Chromium-stable `css/` at **93.4%**, Firefox at **~84.9%**, Safari at **~86%**, and unique single-engine failures of roughly **4,200 (WebKit) / 2,400 (Gecko) / 800 (Blink)** **[V-2nd]**. My computed `/css/` figures are 93.4 / 85.1 / 87.6 — the same to within the test-revision drift. That agreement is the strongest signal I have that the numbers above are sound.

### 3.4 Apple's conformance gate — the number that actually constrains a new engine

Apple's EU entitlement requirements, verbatim **[V]**:

> - Pass a minimum percentage of tests available from industry standard test suites:
>   - **90% of Web Platform Tests**, as a percentage of the highest number of subtests executed by any browser on the wpt.fyi front page; and on an operating system that the test suite is compatible with
>   - **80% of Test262** on an iOS device, iPadOS device, or **Mac with Apple silicon**; and
> - **Meet the above test suite requirement if Just In Time (JIT) compilation is unavailable** (e.g., if Lockdown Mode is enabled by the user)

Plus: memory-safe language for all web-content-facing code, PAC, process separation, vulnerability-disclosure policy, 30-day fix commitments for exploited bugs, third-party-cookie blocking by default, per-site storage partitioning, no cross-app state sync.

Implications:
- **Servo at 88.3% aligned would not clear the 90% bar today** **[C]**, and Servo has no JIT-free product anyway (it depends on SpiderMonkey; Lockdown-style JIT-less operation is a separate engineering problem).
- "Memory-safe language" is a box a Rust engine ticks by construction — that is a genuine, checkable advantage.
- The JIT-off requirement is the one that quietly destroys naive plans: it means an interpreter tier must independently reach 90%/80%.

### 3.5 test262

- **Scope:** ECMA-262 + ECMA-402 + ECMA-404; **over 50,000 test files** (as of May 2025), each possibly containing multiple cases; described in ECMA TR/104, part of the ECMA-414 suite **[V]**.
- **Execution semantics a runner must honour** **[V]**:
  - **a fresh realm per test**; global code unless `module` flag
  - harness files `harness/assert.js` + `harness/sta.js` evaluated first (unless `raw`)
  - host-defined globals: **`print`** (async signalling) and **`$262`** with `createRealm`, `detachArrayBuffer`, `evalScript`, `gc`, `global`, `agent` (for `Atomics`/`SharedArrayBuffer` tests), and optionally `IsHTMLDDA` (to model `document.all`)
  - **each test runs twice** (sloppy + strict) unless `noStrict`/`onlyStrict`/`module`/`raw`
  - `negative` metadata: expected error **phase** (`parse`/`resolution`/`runtime`) and constructor name
  - `flags`: `onlyStrict`, `noStrict`, `module`, `raw`, `async`, `generated`, `CanBlockIsFalse`/`CanBlockIsTrue`, `non-deterministic`; `includes:` for extra harness files; `locale:` for `Intl` tests
  - `test/intl402/` must be **skipped** if you don't implement ECMA-402
- **Runners you can reuse:** `tc39/test262-harness` (Node), `test262-harness-py`, the web runner, C++ and .NET harnesses **[V]**. For a Rust engine, the Python harness is the lowest-friction path: it shells out to an executable that reads a file from stdin and prints the completion token.
- **Pass-rate reporting:** test262 has no official per-engine dashboard; **wpt.fyi now includes test262** in its single-engine-failure charts **[V-2nd]**, which is the closest thing to an official cross-engine view.
- **Known state of the art as a reference point:** Boa (pure-Rust JS engine) claims "more than 90% of the latest ECMAScript specification" and publishes a conformance dashboard **[V]** (from the sibling stack audit). That is the realistic ceiling for a from-scratch Rust JS engine in 2026 — and it is *not* enough for Apple's 80% test262 bar if the remaining 10% includes whole early proposals... except that 90% > 80%, so it *is* enough, if the 90% is measured the same way. **[U]** on the exact measurement alignment.

### 3.6 html5lib-tests, csswg tests, idlharness, reftest vs testharness

- **html5lib-tests** — the repo still exists and is **actively maintained** (last commit 2026-09-29, "Add tokenizer tests for processing instructions") **[V]**. But its README now says: *"The HTML parser tree construction tests are now solely maintained on web-platform-tests"* **[V]**. Repo contents today: `tokenizer/`, `encoding/`, `serializer/` — **no `tree-construction/`** **[V]**. Practical rule: **get tree construction from WPT `html/syntax/parsing`; get tokenizer and encoding tests from html5lib-tests.** Every serious Rust HTML parser (`html5ever`, and Blitz on top of it) is driven by these; the sibling stack audit confirms `html5ever` is "html5lib-test-driven".
- **CSSWG tests** — there is no separate "csswg-test" repo to track any more; CSS tests live in WPT under `css/`, which is the single largest and hardest directory (Chrome 93.4%, Safari 87.6%, Servo 55.4% **[C]**). Interop focus areas are drawn from this pool.
- **idlharness** — auto-generated WebIDL conformance tests: given your `.webidl` files, `idlharness.js` checks that every interface, attribute, operation, and constant exists with the right shape, and that `[Exposed]`, `[Global]`, `[LegacyNoInterfaceObject]` etc. are honoured. **This is the highest-value-per-effort conformance suite for a new engine**: it validates your binding layer before you implement behaviour, and failures are unambiguous. It is also what catches "we have the API but it's on the wrong global".
  > Note: Interop 2026 includes idlharness tests, and there is live friction about idlharness tests that contradict the "supported by all but one" selection rule **[V-2nd]** — i.e. don't assume every idlharness failure is your bug.
- **reftest vs testharness — the operational difference:**
  - *testharness* failures are **diagnosable** (assertion + message) and cheap to bisect.
  - *reftest* failures are **pixel diffs**: you need a screenshot pipeline, a `fuzzy` tolerance policy, and font/AA determinism. A new engine will fail thousands of reftests purely on text rasterisation differences, and every one of those is a judgement call about how much AA/hinting parity you want.
  - **Start with testharness + idlharness; add reftests only after you have a deterministic Metal rasteriser and a font stack pinned to the same fonts the references assume.**

### 3.7 Existing engine scorecards you can publish against

| Scorecard | What it gives you | Status |
|---|---|---|
| **[wpt.fyi](https://wpt.fyi/results/)** | Per-run, per-directory, per-test results for every engine that uploads; the canonical cross-engine view; `/interop-2026` for the focus-area dashboard **[V]** | Live; Servo uploads **daily** (e.g. `Servo 0.7.0-581d527a8`, 2026-10-25) **[V]** |
| **[servo.org/wpt](https://servo.org/wpt/)** | Servo's own focus-area + whole-CSS + whole-WPT pass rates, with a documented scoring rule: score = fraction of passing subtests per test; "subtests" gives equal weight to top-level tests and subtests **[V]** | Live; **the single best template for a new Rust engine's public scorecard** |
| **[webstatus.dev/stats](https://webstatus.dev/stats)** | Feature-oriented view: how many features each engine alone blocks. June 30 2026: **Safari 59, Firefox 43, Chromium 9** **[V-2nd]** | Live |
| **[Edge Top Developer Needs](https://microsoftedge.github.io/TopDeveloperNeeds/)** | Per-feature WPT/test262 status across engines; sums of passed WPT subtests + test262: **Chrome/Edge ~24,000, Firefox ~18,000, Safari ~5,000** of ~25,500 (May 2026) **[V-2nd]** | Live |
| **Interop 2026** | Annual cross-vendor focus areas (anchor positioning, container style queries, dialogs/popovers, scroll-driven animations, view transitions, `attr()`, `contrast-color()`, custom highlights, fetch uploads/ranges, IndexedDB `getAllRecords`, JSPI, media pseudo-classes, Navigation API, scoped custom-element registries, scroll snap, `shape()`, web compat, WebRTC, WebTransport, `zoom`) **[V]** | Announced 2026-02-12; numeric scores are behind a JS SPA I could not scrape **[U]** |

---

## 4. Memory and energy measurement on macOS

### 4.1 Four different "memory" numbers, and why they disagree

| Metric | What it is | Tool | Gotcha |
|---|---|---|---|
| **Virtual size (VSZ)** | Address space reserved, not allocated | `ps -o vsz`, `vmmap` | Meaningless. `vmmap` on a trivial `bash` shows a **385 GB** Guard region **[V]**. Never quote it. |
| **RSS** | Pages currently resident (shared + private) | `ps -o rss`, `top` MEM | **Double-counts shared framework pages across processes.** With N tabs in N processes, summing RSS inflates by gigabytes of shared read-only pages. |
| **Dirty / swapped** | Pages the process has *written* | `vmmap`, `footprint` | The best per-process honesty metric: `vmmap` prints `Dirty and swapped are memory which has been written to by the process` **[V]**. Deduplicates nothing on its own. |
| **Physical footprint** | The kernel's accounting figure; what jetsam/`footprint` emphasise | `footprint`, `vmmap --summary` | **This is the number to publish.** `footprint` "emphasizes memory considered *dirty* by the operating system kernel for purposes of accounting", **de-duplicates multiply-mapped objects**, and reports shared objects separately **[V]**. |

Verified output shapes on this machine (macOS 27.0.1, M4) **[V]**:

```
$ vmmap --summary $$
Physical footprint:         1408K
Physical footprint (peak):  1408K
...
                                VIRTUAL RESIDENT    DIRTY  SWAPPED VOLATILE   NONVOL    EMPTY   REGION
REGION TYPE                        SIZE     SIZE     SIZE     SIZE     SIZE     SIZE     SIZE    COUNT
===========                     ======= ========    =====  ======= ========   ======    =====  ======= 
Guard                            385.0G       0K       0K       0K       0K       0K       0K        1
```

Useful `footprint` flags for a benchmark harness **[V]**: `-p <name|pid>` (repeatable), `-x` (exclude), `-t` (include children — **essential for a multi-process engine**), `-j out.json` (JSON), `-f bytes|formatted|pages`, `--sort`, `--minFootprint MiB`, `--forkCorpse`, `-a` (all processes).
`footprint` "must be run as root when inspecting processes that are not owned by the current user" **[V]** — plan for that in CI.

**The honest per-tab protocol:**
1. Single machine, plugged in, ≥30 min idle, no thermal throttling (`pmset -g therm`), display awake, Wi-Fi off, Spotlight indexing complete.
2. Cold-launch the browser with a **fresh profile**; wait for a settled state (no CPU >2% for 10 s).
3. Open **N = 20** tabs from a *fixed, local, byte-identical* corpus (serve over `http://127.0.0.1` so network variance is zero). Same viewport, same zoom, same `prefers-reduced-motion`, same ad-blocking state, same session-restore behaviour.
4. Measure with `footprint -t -p <browser> -j after.json`, plus `vmmap --summary` for the dirty breakdown.
5. Report **footprint total**, **footprint per marginal tab** (`(F20 − F1)/19`), and **peak**.
6. Repeat 3×, report median; cold-restart between runs.
7. Publish the delta against the *same* protocol on Safari and Chrome on the *same* machine, same day.

**Why published per-tab numbers are not comparable to yours:**
- Chrome is process-per-site-instance; Safari is closer to process-per-tab with a shared WebContent pool; a new engine might be single-process. Under RSS, more processes looks *worse*; under shared-page-aware footprint, more processes can look *better*.
- macOS 27 ships **Safari as a 35 MB stub** (`/Applications/Safari.app` is a symlink into `/System/Cryptexes/App/…`) and **WebKit has no on-disk Mach-O at all** — `find /System/Library/Frameworks/WebKit.framework -type f` returns nothing, i.e. the binary is in the dyld shared cache **[V]**. So WebKit's real code cost is invisible to `du`, `footprint`, and per-process RSS. Chrome, by contrast, ships its own **494 MB framework binary inside a 1,437 MB bundle** **[V]**. Any "our engine is smaller/lighter" claim must acknowledge that the incumbent's engine is partly free because the OS already paid for it.

### 4.2 Energy on macOS: three tools, one of which is a real measurement

**`powermetrics`** — the only tool that reports actual estimated power. Requires root: `powermetrics must be invoked as the superuser` **[V]**. Key samplers/options **[V]**: `-s/--samplers` (comma list; `all`, `default`), `-i` interval (default 5000 ms), `-n` sample count, `-f plist|text` (plist is NUL-separated — parse accordingly), `-o` file, `--show-process-energy` (per-process "energy impact number"), `--show-process-gpu`, `--show-process-coalition`, `--show-process-qos`, `--show-usage-summary`, `-r composite|wakeups|cputime|pid`, `-t` (assumed wakeup cost in µs).
It reports "estimated power consumed by various SoC subsystems, such as CPU, GPU, ANE" **[V]**.

> **Apple's own caveat, verbatim:** *"Average power values reported by powermetrics are estimated and may be inaccurate - hence they should not be used for any comparison between devices, but can be used to help optimize apps for energy efficiency."* **[V]**
> That is the same machine, same tool, same workload — different *devices* is disallowed. Comparing two engines on the *same* device is therefore the only defensible use, and only as a relative optimisation signal, not as an absolute claim.

**Activity Monitor "Energy Impact" / `top -o cpu` POWER.** Both are the same undocumented composite. Mozilla's own performance documentation states: *"Unfortunately 'Energy Impact' is not a good measure for either users or software developers and it should be avoided."* **[V]** It documents the inputs — CPU time, **wakeup frequency (weighted heavily)**, QoS class, disk read/write bytes, GPU time, network packets — with machine-model-specific coefficients read from `/usr/share/pmenergy/Mac-<board-id>.plist` **[V]**.

I verified this mechanism still exists on macOS 27.0.1 / M4 **[V]**:
```
$ ls /usr/share/pmenergy | wc -l
35
$ plutil -p /usr/share/pmenergy/Mac-27ADBB7B4CEE8E61.plist
{ "energy_constants" => {
    "kcpu_time" => 1  "kcpu_wakeups" => 0.0002
    "kgpu_time" => 1.2  "kqos_background" => 0.8  "kqos_default" => 1
    "kdiskio_bytesread" => 1.1e-08  "kdiskio_byteswritten" => 1.9e-09
    "knetwork_recv_packets" => 4e-06  "knetwork_sent_packets" => 4e-06 ... } }
```
Two findings worth carrying: (a) the 2015 plist mechanism is **unchanged in principle 11 years later**; (b) the documented `ioreg -l | grep board-id` lookup **no longer works on Apple Silicon** (no `board-id` key), so the M4's own coefficients are not reachable by that method **[V]**. `top`'s POWER column is `((used_us + IDLEW*500) * 100)/elapsed_us`, i.e. a CPU percentage with a wakeup tax — unitless and not power **[V]**.

**A reproducible energy protocol that survives review:**
1. **Preferred:** a **physical power meter** (USB-C inline, or a wall meter on a Mac mini/Studio) — the only method whose unit is watts. Report `J` for a fixed workload, and `J` per minute of a fixed 1080p video loop through a local HTTP server.
2. **Second best:** **battery-drain A/B** on a laptop. Full charge, identical brightness, Wi-Fi off, airplane-ish, run the fixed workload until a fixed battery threshold (e.g. 50% → 20%), report minutes and derive `W` from the battery's Wh. n ≥ 3, interleaved A/B, report medians and range.
3. **Acceptable as a relative signal on one machine:** `powermetrics -s cpu_power,gpu_power -i 1000 -n 300 -f plist`, integrating the CPU+GPU power samples over the run; report the **ratio** to a reference engine on the same machine, same day, same build conditions. Never report it as watts-of-truth or compare across machines (Apple forbids it **[V]**).
4. **Never** as the headline: Activity Monitor "Energy Impact" or `top` POWER. If you must, report it *alongside* a real measurement and say it is a wakeup-weighted composite **[V]**.
5. **Control the things that dominate laptop energy but aren't the engine:** display brightness and refresh rate, `ProMotion`/LFC state, whether the video path uses hardware decode, timer coalescing, background tab throttling, and whether the machine is on battery vs AC.

**What "Firefox is an energy hog on Apple Silicon" actually measures.** The publicly available numbers are almost all Activity Monitor "12-hour Power" or `top` POWER — i.e. **wakeups and CPU%, weighted by a per-model coefficient**. Firefox has historically had a higher timer/wakeup rate and more separate processes than Safari. Magic Lasso's January 2026 test reports Average Energy Impact: Edge 3236.83, Chrome 3593.27, Firefox 3639.51, **Safari 4271.39** — i.e. *Safari last*, which they attribute to its MotionMark lead **[V-2nd]**. Read that as: the metric partly measures "how hard the GPU was pushed", so a graphics benchmark victory becomes an energy loss. **A composite that rises with useful work cannot rank engines.** That is the critique, and it is decisive.

Practical consequence for your project: the honest energy claim you can make in year one is **"equal work, less energy, measured externally"** — e.g. same page corpus, same time, wall-meter joules. Anything derived from Activity Monitor is refutable in one sentence.

---

## 5. How to prove "better than Blink/WebKit/Gecko" honestly

### 5.1 The measurement matrix (8 rows)

Targets assume: M4-class machine, macOS 27, arm64-only release build, SpiderMonkey-class JS engine embedded (per the sibling stack audit's `mozjs` recommendation), 12 months of work by a small team.

| # | Metric | Instrument & protocol | **v0.1 target (12 mo)** | Best incumbent on same machine | Achievable in 12 months? |
|---|---|---|---|---|---|
| 1 | **Cold start → first interactive window** (ms, p50/p95, n=30) | `os_signpost` around launch; measure from `exec` to first paint + first input dispatch. Fresh profile, 10 min idle, cold FS cache (`sudo purge`). | **p50 ≤ 400 ms**, p95 ≤ 700 ms | Safari ~<1 s, Chromium 1–2 s (structural, not measured here) **[U]** | **YES — the most winnable row.** Incumbents pay for profile load, session restore, extension init, and a dozen background services. You can be genuinely, reproducibly faster here, and it is the number users feel. |
| 2 | **First Contentful Paint** on a fixed 50-page local corpus (ms, p50) | `PerformanceObserver`/`largest-contentful-paint`-equivalent; local HTTP server, cold cache per page, n=20 pages × 3 | **≤ 1.3× Chrome**, i.e. within 30% | Chrome (best-in-class networking + preload scanner) | **PARTLY.** Parity-within-30% on a *curated* corpus is realistic. Beating Chrome on general web FCP is **not** — it needs a preload scanner, speculator, HTTP/2-3 stack, and years of real-page tuning. |
| 3 | **Speedometer 3.1** (median of 5, full 32 workloads) | `crossbench speedometer --repeat=5 --network=<local>`; report per-workload breakdown, not just the aggregate | **≥ 25** (≈60% of Safari 26.2's 41.9) | 41.9 (Safari 26.2, M2) / ~52 (Chrome, M4) / 61 (Chrome, M5) **[V-2nd]** | **YES at 25, NO at 40.** Achieving 25 requires the full stack working (shadow DOM, contenteditable, canvas, SVG, iframes) *and* a JIT-class JS engine. With an interpreter-only JS engine, expect **8–15**. |
| 4 | **MotionMark 1.3.2**, large viewport, 60 Hz | Manual + scripted; display forced to 60 Hz; report subtest scores | **≥ 1,500** (≈ Firefox's 1,497 on M2) | 8,088.73 (Safari 26.2) **[V-2nd]** | **NO — do not target parity.** A Metal-native compositor gets you to 1–3k in a year. 8k is a decade of raster/compositing tuning. **Publish this row as a deficit, honestly.** |
| 5 | **WPT aligned pass rate** (overall + `/dom/` + `/html/` + `/css/`) | `wpt run` + `--log-wptreport` + `update-expectations`; publish both denominators | **≥ 55% overall; ≥ 80% `/dom/`; ≥ 75% `/html/`; ≥ 40% `/css/`** | 97.1 / 94.5 / 93.5 (Chrome/Firefox/Safari) **[C]** | **YES at 55%, NO at 90%.** 90% is the Apple gate and is a 3–5 year, well-funded effort. Do not promise it in 12 months. |
| 6 | **Memory @ 20 tabs** (physical footprint, MB, + per marginal tab) | `footprint -t -p <app> -j`; fixed local corpus; cold profile; n=3 | **≤ 1.15× Chrome**, and **≤ 90 MB/marginal tab** | Chrome/Safari: measure, don't guess **[U]** | **YES — genuinely winnable.** A single-process or few-process engine with an arena-allocated DOM will beat Chromium's per-site-instance overhead on tab count. **But say what you traded**: process isolation is the thing you gave up, and it is a security property, not a nicety. |
| 7 | **Energy per minute of 1080p video** (J/min) | **External power meter** preferred; battery-drain A/B acceptable; `powermetrics` only as a same-machine ratio, never absolute **[V]** | **parity ±10% with Safari** | Safari | **MEASURABLE, HARD TO WIN.** If you use VideoToolbox + a zero-copy display path you can hit parity. Winning needs the whole compositor to avoid waking the GPU unnecessarily. |
| 8 | **Binary size on disk** (arm64-only, stripped, no dSYM) | `du -sm`, plus the main framework Mach-O size | **≤ 120 MB** | Chrome: 1,437 MB bundle / 494 MB framework **[V]**. Safari: not attributable — **35 MB stub, WebKit in the dyld shared cache** **[V]** | **YES, and it is a real, defensible win** — provided you state the Safari caveat. A Rust engine with no bundled V8 and no multi-locale resources can plausibly land under 100 MB. |

Optional 9th row if the JS engine is shared with Firefox: **JetStream 3.0** (target ≥150). Fold it in as a companion to row 3 rather than a headline, because it measures JS+Wasm, which is your dependency's score, not yours.

### 5.2 Statistical methodology (what reviewers will demand)

- **Warmup.** Discard the first iteration. JetStream 3 already separates "startup" (first iteration) from average case **[V]**; Speedometer has a warm-up suite and a `warmupBeforeSync`/`useWarmupSuite` param **[V]**; MotionMark has an explicit warm-up phase before the measured ramps **[V]**. If you report a single number that includes cold-start JIT tiers, say so.
- **Medians, not means.** All three suites already reduce with geometric means across subtests, so the aggregate is already log-scale friendly. Report **median of ≥5 runs** for Speedometer (it is short), **≥3** for MotionMark and JetStream (they are long). Report the **full distribution** (min, p25, median, p75, max) in an appendix.
- **Confidence intervals.** Bootstrap over runs (10k resamples) for the median; report a 95% CI. MotionMark already bootstraps its own change-point CI internally **[V]** — quote it, don't replace it. **If your CI overlaps the incumbent's, you have not shown a difference.**
- **Machine state.** Plugged in; ≥30 min idle; `pmset -g therm` clean; no thermal throttling events; automatic updates off; Spotlight indexing complete; single logged-in user; no other apps; display at **60 Hz** for MotionMark **[V]**; Wi-Fi off with a local HTTP server for all page loads; **10-minute cooldown** between harness runs; record SoC power/temperature in the results file so a reviewer can detect throttling after the fact.
- **Version pinning.** Record: macOS build, engine git SHA, JS engine version (e.g. SpiderMonkey 153.0), GPU driver, display mode, and the **exact benchmark version + WPT revision**. A score without these is an anecdote.
- **Same-day, same-machine, interleaved A/B.** Never compare your run today to a vendor's run from March. Run all engines you intend to name, on your machine, in one session, interleaved (A,B,C,A,B,C,…) to cancel drift.
- **Pre-register the claim.** Write down the metric, the direction, and the threshold *before* running. Post-hoc metric selection is how benchmark marketing happens.

### 5.3 The JS-engine problem — the thing that invalidates most "we beat Chrome" claims

Speedometer 3.1 is a whole-pipeline benchmark, but a large fraction of its wall time is JS execution and DOM-binding overhead. If your engine embeds **SpiderMonkey 153** and Chrome embeds **V8**, then

> (*your engine* − *Chrome*) = (*your DOM/CSS/layout/paint* − *Blink's*) + (*SpiderMonkey* − *V8*)

and you cannot separate the two terms from a single aggregate number. Three ways out, in descending order of rigour:

1. **JS-engine-matched comparison (best).** Because `mozjs` gives you SpiderMonkey 153 — the same engine Firefox ships — you can run **Firefox 158** and **your engine** on the same machine and say: *"with the JS engine held constant, our browser-engine delta on Speedometer 3.1 is X%."* That is a clean, falsifiable, defensible claim about **your** code. It is also the *only* incumbent for which this is possible in Rust today (JSC bindings exist but are WKWebView-shaped, and V8 via `deno_core` is a different embedding model) — see the sibling stack audit.
2. **Per-workload decomposition (good).** Publish all 32 Speedometer workloads. Workloads like `NewsSite-Next`, `Editor-CodeMirror`, `Charts-chartjs` are style/layout/paint-heavy; `TodoMVC-React-Redux`, `proxy-vue` are JS/framework-heavy. Show *where* you win. Reviewers accept "we are 2× faster on the two news-site workloads and 0.6× on the React ones" far more readily than a single number.
3. **Held-constant harness (weak but honest).** Report both a "shell" number (JetStream 3 in your engine shell, no DOM) and a "browser" number, and state the JS engine version in the same sentence as the score.

**Three claims to never make:**
- ❌ "Our engine is faster than Chrome" from a Speedometer aggregate with a different JS engine.
- ❌ "We use less memory than Safari" from RSS or from summed per-process memory.
- ❌ "We are more energy-efficient" from `powermetrics` ratios or Activity Monitor — Apple's own documentation forbids the former for comparison and Mozilla calls the latter unusable **[V]**.

### 5.4 What you *can* say after 12 months, and what you should not

**Defensible (given the matrix above):**
- "Fastest cold start of any browser on this Mac (p50 320 ms vs 1.1 s for Chrome, n=30, same machine, same day)."
- "Within 30% of Chrome's FCP on a fixed 50-page local corpus."
- "25.1 on Speedometer 3.1 — 60% of Safari 26.2's score on the same hardware, with the same JS engine Firefox ships."
- "55% aligned WPT pass rate, 82% on `/dom/`, from a codebase that is 100% memory-safe Rust."
- "90 MB per additional tab at 20 tabs, 1.15× Chrome's figure, in a single-process model."
- "120 MB on disk versus Chrome's 1.44 GB."
- "Parity with Safari on joules-per-minute of hardware-decoded video, measured with an external meter."

**Not defensible:**
- Any MotionMark victory claim in year one.
- Any "beats Safari on energy" claim derived from an OS composite metric.
- Any WPT percentage without stating numerator, denominator, WPT revision, and whether JIT was on.
- Any cross-machine comparison of `powermetrics` power **[V]**.

### 5.5 The 12-month reality check

If the goal is *"prove we beat the best"* in the literal sense — higher Speedometer than Chrome, higher MotionMark than Safari, higher WPT than Blink — **that is not a 12-month goal, and in the case of MotionMark and WPT it is not a 5-year goal either**; Blink and WebKit are 20-year-old codebases with hundreds of full-time engineers. What *is* achievable in 12 months is a **narrow, precisely-scoped, honestly-labelled set of wins** (cold start, binary size, memory-per-tab, and a JS-engine-matched Speedometer delta), plus **credible deficit reporting** on the rows you lose. The second half is what makes the first half believable — and it is exactly what the incumbents' own marketing omits.

---

## 6. Things I could not verify (do not quote these as fact)

| # | Item | Status |
|---|---|---|
| 1 | **Safari's Speedometer 3.1 score on M4** — no vendor figure exists that I could find | **[U]** — must be measured |
| 2 | Chrome's 52.35: Speedometer **3.0** or **3.1**? | **[U]** — Google's post (redirect-blocked) says 3.1; MacRumors, same day, says Google "appears to have used Speedometer 3" **[V-2nd]** |
| 3 | MotionMark 1.3.2 exact release date | **[U]** — version history verified, date not shown |
| 4 | Whether Chrome/Safari **headless** produces MotionMark/Speedometer scores comparable to on-screen | **[U]** — plausible that headless `BeginFrame` scheduling differs; must be validated before any CI number is published |
| 5 | WebXPRT 5 automation/CLI path | **[U]** — none documented |
| 6 | Basemark Web 3.0 current version, maintenance status, automation | **[U]** — page live at `web.gpuscore.com`, titled "Basemark Web 3.0"; no changelog found |
| 7 | Interop 2026 numeric per-browser scores | **[U]** — dashboard is a JS SPA; `/results/api/interop-2026/scores` returned HTML |
| 8 | `crossbench` module id for JetStream 3 | **[U]** — `jetstream_2_2.py` confirmed; a JetStream-3 module was not confirmed |
| 9 | Magic Lasso's numbers (n=3, single M2 Air, vendor-published, no CIs) | **[V-2nd]** — treat as indicative, not authoritative |
| 10 | iPhone/Blink prototype numbers | **[V-2nd]** — primary is an Edge engineer's LinkedIn post, relayed by OWA; I did not read the primary |
| 11 | Per-tab memory figures for Safari/Chrome on macOS 27 | **[U]** — not measured here; the protocol in §4.1 is what you'd run |
| 12 | Whether any engine actually passes Apple's 90%/80% gate **with JIT disabled** | **[U]** — the requirement is verified; compliance data is not public |

---

## 7. Sources

**Benchmarks & versions**
- https://browserbench.org/ — current suite index (Speedometer 3.1, JetStream 3.0, MotionMark 1.3.2)
- https://browserbench.org/announcements/speedometer3.1/ — Speedometer 3.1, 2025-03-31
- https://browserbench.org/announcements/jetstream3/ — JetStream 3.0, 2026-03-31
- https://browserbench.org/Speedometer3.1/about.html — workload list and methodology
- https://browserbench.org/Speedometer3.1/instructions.html — run hygiene
- https://browserbench.org/JetStream3.0/in-depth.html — 77 workloads, scoring, version-incomparability
- https://browserbench.org/MotionMark1.3.2/about.html — subtests, bootstrap CI, 60 Hz requirement, version history
- https://webkit.org/blog/17899/introducing-the-jetstream-3-benchmark-suite/ — JetStream 3 design + M4 numbers (359 → 395)
- https://www.principledtechnologies.com/benchmarkxprt/webxprt/ — WebXPRT 5 scenarios, AI workloads
- https://benchmarks.ul.com/news/procyon-essentials-launch — Procyon Essentials, 2026-05-05
- https://developers.google.com/octane → 301 → https://browserbench.org/JetStream/ — Octane retirement
- https://web.gpuscore.com/ — Basemark Web 3.0 current host

**Reference numbers**
- https://www.magiclasso.co/insights/best-web-browser-2026/ — M2 MacBook Air, 2026-01-08: Speedometer 3.1 / JetStream 2.2 / MotionMark 1.3.1 / Energy Impact
- https://www.macrumors.com/2025/06/05/chrome-speedometer-improvements/ — Chrome 139 = 52.35 on M4 MBP, version caveat
- https://www.macrumors.com/2026/06/05/chrome-browser-speed-record-2026/ — Chrome on M5 MBP: Speedometer 61, JetStream 3 = 469
- https://open-web-advocacy.org/blog/28-percent-faster--the-blink-prototype-that-shows-why-apples-ios-browser-engine-ban-must-end/ — iPhone 17 Pro Max Blink vs Safari on all three suites

**Conformance**
- https://web-platform-tests.org/test-suite-design.html — test types (reftest / testharness / wdspec / aamtest / manual)
- https://web-platform-tests.org/running-tests/from-local-system.html — `wpt run`, hosts file, `--log-wptreport`, `--processes`
- https://web-platform-tests.org/tools/wptrunner/docs/expectation.html — `.ini` metadata, `expected`/`disabled`/`fuzzy`/`restart-after`, `update-expectations`
- https://web-platform-tests.org/running-tests/custom-runner.html — manifest as canonical enumeration
- https://github.com/tc39/test262 — suite scope, >50,000 files, runners
- https://github.com/tc39/test262/blob/main/INTERPRETING.md — realm isolation, `print`/`$262`, strict/sloppy, `negative`, flags
- https://raw.githubusercontent.com/html5lib/html5lib-tests/master/README.md — tree-construction moved to WPT
- https://wpt.fyi/results/ and https://wpt.fyi/api/runs — run inventory and summary artifacts
- https://servo.org/wpt/ — the scorecard template to copy
- https://developer.apple.com/support/alternative-browser-engines/ — 90% WPT / 80% test262 / JIT-off gate
- https://web.dev/blog/interop-2026 — Interop 2026 focus areas
- https://infrequently.org/2026/07/abject-praise/ — WPT per-directory rates, single-engine failure counts, 90% gate context

**macOS measurement**
- `man footprint`, `man vmmap`, `man powermetrics`, `man leaks`, `man heap` (local macOS 27.0.1) — dirty vs resident, de-duplication, powermetrics caveat
- https://firefox-source-docs.mozilla.org/performance/activity_monitor_and_top.html — "Energy Impact … should be avoided"
- https://blog.mozilla.org/nnethercote/2015/08/26/what-does-the-os-x-activity-monitors-energy-impact-actually-measure/ — Energy Impact formula, `top` POWER, `/usr/share/pmenergy`
- https://chromium.googlesource.com/crossbench/+/refs/heads/main/README.md — canonical multi-browser runner
