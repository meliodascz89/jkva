# Prior art and scope realism for a from-scratch Rust browser engine (Apple Silicon)

**Date:** 2026-10-25
**Question:** is a "complete usable engine" v0.1 in Rust, written from scratch, achievable by a small team of AI agents plus one human — and if so, at what scope and on what timeline?

**Method:** primary sources only where possible — project blogs, status pages, repo metadata via the GitHub API, crates.io download counts, line counts taken from shallow clones of the actual repositories. Where a claim could not be verified it is marked as such rather than smoothed over.

---

## 0. Bottom line up front

The single most useful number in this whole document: **Ladybird's browser-engine core is roughly 1.2 million lines of code** (LibWeb ~830k, LibJS ~357k), it has been worked on since **June 2019**, it is funded by a nonprofit with Cloudflare/Shopify/FUTO as platinum sponsors, and as of **September 2026** — seven years in — it had **not yet shipped its alpha**, and scored **~5.9 on Speedometer 3**, roughly an order of magnitude below shipping browsers.

Blitz, the best-executed "reuse the crates" engine, is **~68–80k lines of Rust** after ~2.5 years of serious work, is explicitly **beta**, renders Wikipedia and old Reddit, has **no JavaScript at all**, and still does not support `overflow: auto`, `position: sticky`, real table layout, or 3D transforms.

Aurora, built in 2026 on maximum reuse (Blitz + Stylo + Vello + V8), reached **~28.6k lines in six months** and its stated benchmark is "make one real YouTube route bootstrap and paint reliably" — explicitly *not* navigation, login, or playback.

So the honest calibration is: a from-scratch engine whose target is "renders the modern web correctly" is a **multi-year, 300k–1M+ line** project even with heroic reuse. A 6-month "complete usable engine" is off by roughly **3–5× in time and 5–10× in code**. The defensible 6-month target is a *static-content engine*; "usable browser" is a 24-month target at best, and only if the scope is defined down aggressively.

The rest of this document substantiates those numbers.

---

## 1. Servo (Rust; Mozilla origin, now Linux Foundation Europe)

### Status in October 2026

Servo is the only Rust engine with real institutional continuity: repo created **2012-02-08**, **58,888 commits**, **38,228 stars**, ~**544k lines of Rust**, MPL-2.0, hosted by Linux Foundation Europe. [Repo metadata via GitHub API](https://github.com/servo/servo)

The current release is **Servo 0.6.0**, covering work landed in August 2026 — **417 commits in one month**. The headline item is that **CSS Grid is now enabled by default** ([@nicoburns, #45621](https://github.com/servo/servo/pull/45621)), which the project describes as having "a huge positive impact on the visual appearance of many websites." The same cycle shipped `touch-action`, `animation-composition`, `color-scheme`, the `full-width`/`math-auto`/`full-size-kana` values of `text-transform`, **interactive text selection**, a disk-backed HTTP cache, SpiderMonkey **v153**, and resvg 0.48. [August in Servo](https://servo.org/blog/2026/10/09/august-in-servo/)

Two things in that post matter for scope realism more than the feature list:

1. **Navigation and session history are called out as "some of the gnarliest parts of the HTML specification"**, with a link to hsivonen's essay on why `about:blank` is complicated. A 14-year-old engine with 30 maintainers is *still* fixing multi-window and iframe navigation in 2026.
2. **Incremental layout dirty-root computation** is still being fixed ("many edge cases around determining dirty roots for incremental layout", #46906). This is the single hardest thing to retrofit and the main Speedometer lever.
3. **Shaping text across inline boxes** needed its own fix (#46829). Text layout across inline boundaries is not a solved problem you inherit from a shaping library.

Performance: an experimental **"Threadboost"** mechanism is reported to improve Speedometer by **50%** and let canvas reach 60 FPS — initially OpenHarmony-only. Note what that implies: Servo needed a dedicated threading mechanism to move the needle on Speedometer *after* fourteen years of optimization.

### What it actually renders today

Servo ships **nightly desktop builds for Windows, macOS (both x86_64 and aarch64), Linux (x64/aarch64), Android and OpenHarmony**. Apple Silicon is a first-class target. The download page's own warning is the most honest maturity statement in the ecosystem: **"Please don't log into your bank with Servo just yet!"** and the macOS `.app` is unsigned. [servo.org/download](https://servo.org/download/)

Servo does not publish a single headline WPT number in fetchable form (the [WPT page](https://servo.org/wpt/) renders its table client-side); results are browsable per-subsystem on [wpt.fyi](https://wpt.fyi/results/?product=servo).

### Architecture

Confirmed by the `components/` directory and the Servo Book: **constellation** (the per-WebView state machine and message router; the closest thing to a "browser process"), **script** (DOM + JS bindings, SpiderMonkey host), **layout**, **paint**, **compositor**, **net**, **media**, **canvas**, **fonts**, plus **webgl/webgpu/webxr/webvtt/webdriver_server/devtools**. WebRender is the GPU renderer but no longer lives in-tree as a component — it is a sibling repo/crate. [Servo Book](https://book.servo.org), [components listing](https://github.com/servo/servo/tree/main/components)

The `script` component's JS integration is worth flagging as a cautionary tale: Servo is on a long-running effort to use Rust's type system to make SpiderMonkey integration safer, is supporting a **large-scale rewrite of the JS engine integration to fix intermittent panics related to garbage collection**, and is *still* adding static prevention for a footgun that produces uncollectible GC cycles. **Rust's ownership model does not solve the DOM/JS cycle problem.** More on this in §7.

### Team and funding — the real constraint

- **30 core maintainers** plus 9 subproject maintainers (core-foundation, euclid, smallvec, url, WebRender). [MAINTAINERS.md](https://github.com/servo/project/blob/main/governance/MAINTAINERS.md)
- **$8,128/month** in recurring donations (+3% month-over-month), 36 thanks.dev sponsors. [August in Servo](https://servo.org/blog/2026/10/09/august-in-servo/)
- That money funds **one part-time maintainer role**. Josh Bowman-Matthews' year-in-review: **1,150 PRs reviewed, 8 new maintainers nominated, 114 newcomer issues filed (92% fixed)**. He describes finding "a healthy balance that allows me to spend time with my family." [One Year of Sponsored Servo Development](https://servo.org/blog/2026/09/15/one-year-of-sponsorship/)
- There is also an NLnet grant for navigation and downloads: [NLnet: Servo Navigation & Downloads](https://nlnet.nl/project/Servo-Navigation_Downloads/)

**Read that ratio carefully.** Servo's throughput comes from ~30 part-time people plus a large volunteer pool, coordinated by one part-time funded reviewer. This is the actual shape of a successful Rust engine project's resourcing — and it is roughly 30× a "1 human + agents" team.

### Which crates are genuinely reusable

This is Servo's most under-appreciated contribution, and the download counts (crates.io API, 2026-10-25) show it:

| Crate | Version | Downloads | Reusable? |
|---|---|---|---|
| `html5ever` | 0.40.1 | **132.9 M** | Yes — drop-in spec-compliant HTML parser (servo/html5ever) |
| `cssparser` | 0.38.0 | **108.0 M** | Yes — tokenizer/parser primitives (servo/rust-cssparser) |
| `selectors` | 0.41.0 | **91.4 M** | Yes — selector matching engine (servo/stylo) |
| `rustybuzz` | 0.20.1 | **36.6 M** | Yes — complete HarfBuzz shaping port; now under the **harfbuzz** org. Shaping only: no font loading/fallback (pair with `skrifa`/`fontdb`/`fontique) |
| `stylo` | 0.22.0 | 444 k | Partly — real CSS cascade/values, but requires you to implement Servo's `style` DOM traits. Blitz carries a dedicated `stylo_taffy` glue crate precisely for this |
| `webrender` | 0.70.0 | **391 k** | **No, not practically** — published, but coupled to Servo/Gecko display-list semantics. Two orders of magnitude less adopted than the parsers for a reason |

The pattern is unmistakable: **parsing, selector matching and shaping are commodities; style resolution, layout and painting are not.** The 391k-vs-133M download gap between `webrender` and `html5ever` is the whole story of reusable browser infrastructure.

`rustybuzz`'s origin, for the record: it began as RazrFalcon's complete port of HarfBuzz's shaping algorithm to Rust and has since moved to the HarfBuzz organisation itself ([harfbuzz/rustybuzz](https://github.com/harfbuzz/rustybuzz)), so it tracks upstream HarfBuzz behaviour rather than drifting from it.

---

## 2. Blitz (DioxusLabs) — the closest analogue, examined closely

Blitz is the most important comparison in this document, because it is exactly the architecture the brief proposes: reuse Stylo for style, Taffy for box layout, Parley for text, Vello/Skia for painting, write the DOM and glue yourself.

### Vital statistics

| | |
|---|---|
| Repo created | **2022-02-28** (prototype era) |
| Serious work begins | ~Jan 2024 (roadmap calls 0.1 "18 months of work", shipped June 2025) |
| One-line desc | "A radically modular HTML/CSS rendering engine" |
| Commits | **2,126** |
| Velocity | **952 commits in the last 52 weeks**; 414 in the last 13 (~32/week); peak week 82 |
| Contributors | **52** |
| Rust LOC | **~68k–80k** (GitHub byte-proxy 68,464; my clone count 80,571 including blank lines — plus ~30k lines of HTML and ~10k of CSS *test fixtures*) |
| Stars / open issues | 4,307 / 321 |
| License | **Apache-2.0 OR MIT**; `stylo_taffy` additionally MPL-2.0 (triple-licensed) for Servo interop |
| Current state | **"beta"** — renders many popular no-JS sites (Wikipedia, old Reddit), usable for apps "if you are an early adopter and willing to live on the bleeding edge" |

Sources: [github.com/DioxusLabs/blitz](https://github.com/DioxusLabs/blitz), GitHub API, [roadmap issue #119](https://github.com/DioxusLabs/blitz/issues/119).

### Architecture

Blitz is genuinely modular, and the crate decomposition is the best public blueprint for this kind of project:

- **`blitz-dom`** — core DOM abstraction: style resolution, layout, event handling. *Uses Stylo (CSS), Taffy (box layout), Parley (text layout).* No parsing, rendering or system integration.
- **`blitz-traits`** — minimal trait/type crate so the others don't depend on each other.
- **`blitz-html`** — adds HTML parsing on top of `blitz-dom`. *Uses html5ever + xml5ever.*
- **`blitz-paint`** — turns the `blitz-dom` tree into `anyrender` draw commands.
- **`blitz-renderer-vello`** — Vello (GPU) or Skia backends.
- **`blitz-shell`** — window/event loop/a11y/menus. *Uses winit, accesskit, muda.*
- **`blitz-net`** — http/file/data fetching via reqwest.
- Wrappers: **`blitz`** (HTML/markdown frontend) and **`dioxus-native`** (VirtualDom frontend with real interactivity).

Note the renderer split: **two backends, Vello and Skia.** That matters because the status page admits `filter` and `backdrop-filter` are only *partially* supported with the Vello Hybrid backend (blur and drop-shadow only) and fully supported only with Skia.

### What it supports

Genuinely working, per the project's own [status page](https://blitz.is/status/css) (percentages are real-world usage frequency, which makes this the best available prioritisation dataset):

- **Layout:** block, inline, inline-block, **flex**, **grid**, **floats + clear**, static/relative/absolute/fixed positioning, full `z-index`, `box-sizing`, `display:contents`.
- **Grid:** named lines, `grid-template-areas`, `fr`/`min-content`/`max-content`/`fit-content()`, `grid-auto-*`. **No subgrid.**
- **Tables:** *emulated with CSS Grid* (`display:table` → "partial support", taffy#467). `border-collapse` partial: assumes uniform border width/style/colour and takes styles from the first cell.
- **Text:** `@font-face`, font-weight/style/stretch, `font-feature-settings`, `font-variation-settings`, `letter-spacing`, `word-spacing`, `text-transform`, `text-decoration` (with thickness/offset/style), `vertical-align`, `white-space-collapse`, `text-wrap-mode`, `word-break`, `overflow-wrap`, `text-indent`, `text-align-last`.
- **Painting:** 2D transforms, transitions, animations, `box-shadow`, `outline`, `clip-path`, `mask-*`, `opacity`, `isolation`, `object-fit`, `border-radius`, all standard `border-style`s.
- **Interaction:** pointer/mouse/scroll/focus/touch events, `pointer-events`, `user-select`, `touch-action`, text selection, `<details>`/`<summary>`, `<iframe>`, custom widget API, AccessKit a11y.

### The gaps — this is the useful part

Taken verbatim from the status page, ordered by real-world usage:

| Gap | Usage | Consequence |
|---|---|---|
| **`overflow: auto`** | 87% bucket | **Not supported.** `hidden`/`scroll`/`clip` are. Every real page has scrollable containers |
| `position: sticky` | 91% bucket | **In progress** (blitz#887) |
| `appearance` | 52% | Not supported — form controls can't be de-native-styled |
| `color-scheme` | 24% | Not supported |
| `will-change` | 42% | Not supported |
| `text-rendering` | 34% | Not supported |
| `text-overflow` | 66% | **In progress** (blitz#893) — no ellipsis yet |
| `line-clamp` | 44% | Not supported |
| **3D transforms** | 84% bucket | Entirely unsupported: `transform`, `perspective`, `backface-visibility`, `transform-style` |
| **SVG styling** | 66% | `fill`, `stroke`, `stroke-width`, `fill-rule` etc. all unsupported. SVGs *render*; they can't be styled |
| `text-shadow` | 46% | Not supported |
| `mix-blend-mode` | 14% | Not supported |
| `counter-increment`/`reset` | 11%/9% | Unsupported — no CSS counters |
| Multi-column | 6% | Entirely unsupported |
| Container queries | 21% | Unsupported |
| Anchor positioning | 6% | Unsupported |
| `scroll-snap-*` | 19% | Unsupported |
| `overscroll-behavior` | 22% | Unsupported |
| `unicode-bidi` | 17% | Unsupported; `direction` is **block-level only**, not inline; `writing-mode` partial |
| `hyphens` | 18% | Unsupported |
| `font-display`, `font-kerning`, `font-synthesis`, `font-size-adjust` | 53/3/3/2% | Unsupported |
| `content-visibility` / `contain-intrinsic-size` | 10/9% | Unsupported |
| `content: counter()`/image/gradient | 81% bucket | Only `content: <string>` works |
| `::-webkit-font-smoothing`, `text-size-adjust` | 62/22% | Unsupported |

**There is no JavaScript engine.** Not "limited" — absent by design. This is the single biggest scoping fact about Blitz, and it is why it can be 68k lines instead of 1.2M.

### Velocity and funding

Blitz received an **NLnet NGI Zero Core grant starting 2024-12**, with a scope that is refreshingly honest about how much work "render basic web pages" is: bring Blitz "up to scratch" for use as an HTML/CSS browser, **explicitly excluding JavaScript**; target use cases are *browsing Wikipedia, viewing news websites, and searching using a search engine*; deliverables were **layout improvements, form controls, WPT test infrastructure, and an initial browser UI**. [NLnet project page](https://nlnet.nl/project/Blitz/)

That a funded grant's entire deliverable is "render Wikipedia and a news site, plus form controls and a browser UI, without JS" is the most concrete scope anchor available anywhere in this document.

### Roadmap: what's still not done for 1.0

From [issue #119](https://github.com/DioxusLabs/blitz/issues/119): DevTools via Chrome DevTools Protocol (element tree + picker, layout metrics, specified and computed styles), `position: sticky`, **real table layout**, `contain`, `line-clamp`, `text-wrap-style`, and better **font enumeration/fallback** including `unicode-range` and `font-display`. Millestones were 0.1 (Jun 2025), 0.2 (Oct 2025), 0.3 beta (est. Aug 2026, delivered).

**Blitz is, after ~2.5 years and an NLnet grant, pre-1.0 with no JS, no sticky, no real tables, no `overflow:auto`.** That is the empirical ceiling of the "reuse Stylo + Taffy + Parley + Vello" strategy at this team size.

---

## 3. Aurora — and a correction to the brief

**The brief describes this project as using SpiderMonkey via `js_sm`, and as being called "Drift". Both appear to be wrong as of today**, and it's worth correcting because it changes the lesson.

Per the repo's own README ([JohannaWeb/Aurora](https://github.com/JohannaWeb/Aurora), MPL-2.0, created **2026-03-30**, last push **2026-09-05**, **161 commits**, 90 stars, ~**28.6k lines of Rust**):

> "JavaScript: **V8 only**. Earlier SpiderMonkey and Boa experiments were removed; unsupported `AURORA_JS_ENGINE` values fall back to V8."

And the README's opening self-description is unusually candid:

> "It is **not** a production browser, a spec-complete engine, or a from-scratch implementation of every browser subsystem. Aurora integrates the hard browser parts it needs: **V8** for JavaScript, **blitz-dom/Stylo** for much of DOM/CSS/layout, **blitz-paint plus Vello/WGPU** for rendering, and **reqwest/rustls** for network fetching. The Aurora-owned work is the integration layer, runner, capability model, DOM bridge, rendering glue…"

### Architecture

| Area | Implementation |
|---|---|
| JS | V8 behind a `JsRuntime` trait |
| DOM bridge | Rust-side node registry + partial DOM/BOM bindings |
| Rendering document | `blitz-dom` + `blitz-paint` (**primary path**) |
| Legacy DOM/layout | Hand-rolled parser + style tree + layout tree, still used by tests, screenshots, JS layout accessors |
| GPU | Vello + WGPU via `anyrender_vello` |
| Window | winit + scroll/input + browser chrome |
| Media | optional FFmpeg-backed video frames |

There is also an explicit "capability model" (sandboxed/`ReadWorkspace`/`network` capabilities gating `file://` and URL fetching, runner identity) aimed at "an agent-controlled browser surface" — which is a genuinely interesting design direction and adjacent to this project's premise.

### What it proves

1. **Reuse gets you to pixels fast.** Six months, one person, 28.6k lines → a window, GPU painting, HTML from string/URL/fixture, PNG screenshots, visual-regression fixtures.
2. **Capability gating + a `JsRuntime` trait is a good seam.**
3. **The blockers are integration, not invention.** The project's own stress test is YouTube's Polymer/ShadyDOM architecture; the failure modes it documents are shadow-root composition, custom-element upgrade ordering, event propagation, and a **Stylo panic when an element lacks computed styles** (fixed upstream, unreleased at the time). Its first bug was that the masthead logo laid out at 0×0 because shadow fragments weren't reattached to the connected tree.
4. **Two parallel DOM/layout trees is the classic trap.** The README names it as the top technical debt: a legacy hand-rolled path plus the Blitz render document, with the legacy tree still live in tests, screenshots, JS layout accessors and input.

### What it does not

It does not claim, in its own words: full HTML parser correctness, broad CSS property and layout coverage, browser-grade JS scheduling semantics, full DOM/BOM/Web API/storage/cookie/navigation/media behaviour, security isolation for hostile content, full YouTube rendering, or WPT compliance. Its benchmark is *one* content-bearing YouTube route bootstrapping and painting — **not** navigation, login, or playback.

**Lesson:** the maximum-reuse strategy compresses "blank repo → something that paints a real page" into ~6 months. It does not compress "something that paints a real page → a browser." Those are different projects separated by ~10× the work, and the second one has no shortcut.

---

## 4. Ladybird (LibWeb/LibJS, C++) — the calibration case

Ladybird is the only modern, independent, *funded* engine to attempt the full problem, and it is therefore the best available measurement of what "complete engine" costs.

### Timeline

| Date | Event |
|---|---|
| **June 2019** | Andreas Kling starts LibHTML inside SerenityOS ("Start working on a simple HTML library") — per [W3C TPAC 2024 breakout](https://www.w3.org/2024/09/TPAC/breakouts.html) |
| Sept 2022 | Spun out as a standalone project |
| 30 Jun 2024 | Kling steps back from SerenityOS to focus solely on Ladybird |
| Jul 2024 | Ladybird Browser Initiative funded; **Chris Wanstrath (GitHub co-founder) puts in $1M** ([heise](https://www.heise.de/en/article/Ladybird-web-browser-takes-off-One-million-US-dollars-from-GitHub-founder-9789840.html)) |
| Sept 2024 | Original public plan: **alpha 2026, beta 2027, stable 2028** ([W3C talk](https://www.w3.org/events/meetings/8e1ca708-fdbf-4264-a79b-4c953fa85248/)) |
| Mar 2025 | 4th on WPT, behind Chrome/Safari/Firefox; 2nd-most-conformant JS engine after SpiderMonkey |
| Feb 2026 | JS parser + bytecode generator ported **C++ → Rust**, assisted by Claude Code and OpenAI Codex ([adopting Rust](https://ladybird.org/posts/adopting-rust/)) |
| 5 Jun 2026 | **Stops accepting public pull requests** from non-maintainers, citing contribution quality and security ([post](https://ladybird.org/posts/changing-how-we-develop-ladybird/)) |
| Sept 2026 | Style + layout moved to Rust; HTML parser in Rust; Trail of Bits security audit; **alpha still not shipped** |

Sources: [ladybird.org](https://ladybird.org/), [Wikipedia](https://en.wikipedia.org/wiki/Ladybird_(web_browser)).

**Seven years and three months from first commit, and as of today (2026-10-25) the alpha is still "coming later this year" for macOS and Linux only.**

### Size

Measured from a shallow clone of `LadybirdBrowser/ladybird` (LibWeb + LibJS only, excluding tests):

| Component | C++ | Rust | Total |
|---|---|---|---|
| **LibWeb** | 311,734 (.cpp) + 92,446 (.h) = 404k | **425,679** (.rs) | **~830k** |
| **LibJS** | 15,052 | **342,365** (incl. `Flap` JIT) | **~357k** |
| **Engine core** | | | **~1.19 M** |

Whole-repo language bytes (GitHub API, byte proxy) put this at ~1.8M code lines plus **774k lines of HTML** test fixtures. The single largest files are instructive: LibWeb `css/style/tests.rs` 12,221 lines, `css/animation.rs` 9,744, `css/style_compute.rs` 8,721, `css/style/selector.rs` 8,228, `layout/grid_formatting_context.rs` 6,129; LibJS `bytecode/codegen.rs` 10,710.

**If you want a number for "complete usable engine," it is ~1.2M lines of engine code — and that engine still isn't at alpha.**

### Team and funding

- "A small **full-time engineering team**"; team size "stable for now"; policy of **maintaining 18 months of runway at all times**. [ladybird.org FAQ](https://ladybird.org/#faq)
- Sponsors: **Platinum ($100k/yr) FUTO, Shopify, Cloudflare**; Gold ($50k) Human Rights Foundation, Proton VPN; Silver ($10k) 37signals, Axeptio, Mitchell Hashimoto, Paul Copplestone; plus ~20 Copper ($1k) and infrastructure sponsors (Blacksmith, OakHost). All "unrestricted donations", no board seats.
- Funded entirely by donations — no search deal, no monetisation.

So: call it **5–10 full-time-equivalent engineers for ~5 years** plus the SerenityOS volunteer inheritance. That is the real denominator.

### What the pre-alpha actually contains (Sept 2026 state)

From the [September 2026 newsletter](https://ladybird.org/newsletter/2026-09-30):

- **Security architecture:** renderer compromise must not be game over; cookies handled by the network process for HTTP and WebSocket; renderer terminated if it requests privileged access; **cross-site iframes in separate processes**; Landlock filesystem restrictions on Linux; hardened runtime on macOS. Trail of Bits found a "considerable security backlog" and crafted full exploit chains.
- **Graphics:** macOS compositor frames go straight to a native display layer (removing a Metal pass); hardware video decode via VideoToolbox (H.264/HEVC/VP9/AV1); separate **media process**; off-main-thread compositor animations for opacity/transform/background-color/filter; **OffscreenCanvas 2D/WebGL/WebGL2 in workers**.
- **CSS:** `scroll-state()` queries, `content-visibility`, `contain-intrinsic-size`, CSS `@container`/`@scope`/`subgrid` (from May), force-dark via Oklab.
- **Browser chrome:** Qt-only frontend, crash reporter, task manager, private browsing, profiles, content-blocking filter lists with auto-update, DevTools.
- **Perf, honestly reported:** monthly gains of **+30% Speedometer 2, +50% Speedometer 3, +80% StyleBench**. Absolute Linux CI numbers: **Speedometer 2 ≈ 84, Speedometer 3 ≈ 5.9, StyleBench ≈ 155.**

That last line is the most sobering datum in this document. After seven years, a funded team, 1.2M lines, and a 50% month-over-month improvement, **Speedometer 3 is ~5.9**. For reference, shipping desktop browsers run roughly an order of magnitude higher on this benchmark. Ladybird is *correct enough to be useful* and *nowhere near competitive on interactivity*.

- **WPT:** **2,109,072 passing subtests**, +20,395 in September. Roughly half that gain came from tests added upstream that month.

### Memory/event-loop bugs worth memorising

Also from September: background tabs reached **11 GB (Politico, 4h)** and **15 GB (El País, 20h)** because replaced documents were never collected — fixed by releasing references held by navigation requests and pending rAF/idle callbacks. And an ad library's `setTimeout(...,0)` chain pegged the CPU because **timer nesting level wasn't carried between callbacks**, so the spec's 4 ms clamp never applied.

These are the bugs you ship in year seven. They are not layout bugs. Budget for them.

---

## 5. Other projects worth knowing

| Project | Language | What it is | Lesson for us |
|---|---|---|---|
| **Blitz / `blitz-dom`** | Rust | See §2. `blitz-dom` is separately usable as a Stylo+Taffy+Parley DOM without the shell | The modular decomposition is the reusable artefact, even more than the code |
| **verso** ([tauri-apps/verso](https://github.com/tauri-apps/verso)) | Rust | Browser shell built *on Servo*; explores multi-view/multi-window, then building UI in Servo itself. README: "At the moment, Servoshell should provide a better user experience" | Building a *shell* is its own multi-year project; don't conflate shell with engine |
| **servoshell / minibrowser** | Rust | Servo's reference embedder; nightly builds for macOS arm64 | This is what "usable browser shell" looks like after 14 years: unsigned nightly, "don't log into your bank" |
| **Aurora** | Rust | §3. Integration layer over Blitz + V8 | Max reuse → prototype in 6 months, not engine |
| **Ladybird** | C++/Rust | §4. The full-attempt calibration case | 1.2M LOC, 7 years, alpha pending |
| **Lexbor** ([liblexbor 3.0.1 in Debian](https://packages.debian.org/forky/liblexbor-dev)) | C | HTML/CSS parsing + DOM + serialization library, embeddable; used for HTML→image and as a parser in other engines | Confirms the parser layer is a commodity you should never write |
| **Ultralight** ([ultralig.ht](https://ultralig.ht/)) | C++ | "Universal, pure-GPU HTML renderer", 2.0; proprietary/commercial licence | Proves a *commercial* non-browser HTML renderer is viable — but it is explicitly not a browser and buys its speed by scoping HTML/CSS down |
| **html5ever-based hobby engines** | Rust | Many start; almost all stop at the layout stage | The graveyard is real; the parse is the easy 5% |
| **Nyx** | — | **Could not verify.** No credible primary source (repo, site, or announcement) surfaced in searches; one result was an unrelated domain-squat | Treat as unverified; do not cite |

---

## 6. Scope calibration

### What the evidence actually supports

| Data point | Effort | Output | Ratio |
|---|---|---|---|
| Aurora | 1 person, ~6 months, max reuse | 28.6k LOC, fixtures + partial YouTube bootstrap, no spec compliance | ~5k LOC/person-month |
| Blitz | ~2.5 yrs serious, 52 contributors, NLnet grant | 68–80k LOC, no JS, beta, no `overflow:auto`/sticky/tables | ~2.5k LOC/person-month amortised |
| Servo | 14 yrs, 30 maintainers, ~$8k/mo | 544k LOC, CSS Grid only in 2026 | — |
| Ladybird | 7 yrs, funded team, 1.2M LOC | alpha pending, Speedometer 3 ≈ 5.9 | — |

Assume an AI-agent-heavy team reaches Aurora's rate or better — say **8–12k LOC/person-month of *engine* code** (not tests, not fixtures). Then:

- **6 months → ~50–70k LOC.** That is exactly Blitz's current size. Which means: at six months you will have rebuilt Blitz, approximately, minus the polish.
- **12 months → ~100–140k LOC.** Roughly Blitz + a year. Still no JS at scale.
- **24 months → ~200–280k LOC.** Between Blitz and Ladybird-pre-alpha in size, i.e. *maybe* where Ladybird was in 2023–24.

**Conclusion: "complete usable engine in a v0.1 at 6 months" is not achievable.** The credible 6-month deliverable is a static-content renderer; "usable browser" is a 24-month aspiration and even then only with a hard scope cut (see below).

### 6 months — "renders the static web, mostly correctly"

Target: **a curated corpus of 50–100 real content sites renders correctly and fast, with no JS, no forms, no iframes.** This is Blitz-0.1 territory. ~50–70k LOC.

**Must have (in build order):**
1. HTML parsing via `html5ever`; DOM tree with the tree-construction edge cases you get free but must wire correctly (`<table>` foster parenting, `<template>`, implied `</p>`, foreign content).
2. `cssparser` + `selectors` + `stylo` (or hand-rolled cascade if Stylo's DOM traits prove too invasive) → computed style.
3. Box tree construction: anonymous boxes, `display` fixup, `::before`/`::after` with string content (81% of pages).
4. **Block layout**: margin collapsing (all four cases), BFC, containing-block chains, `box-sizing`, min/max sizing, `width:auto` shrink-to-fit for floats/inline-block.
5. **Inline layout** — the hard one: line breaking, `white-space` collapsing, `vertical-align`, inline backgrounds/borders, `text-decoration` propagation, shaping *across* inline boundaries. Drive `parley` for shaping/bidi, but the line-breaking-to-boxes algorithm is yours.
6. **Flexbox** via `taffy` (or embed taffy wholesale for block+flex and layer inline on top).
7. **Floats + clear** — floats interacting with inline content and BFC establishment.
8. `position: static/relative/absolute/fixed`; `z-index` and **stacking contexts** including the `opacity`/`transform`/`filter` context-creating properties.
9. Painting via `vello` or `vello_cpu`: backgrounds (colour, image, position, size, repeat, attachment), `border-radius` with the correct corner-joining maths, `box-shadow`, `outline`, `clip-path`, `mask`.
10. Text painting: `@font-face` (72%), font matching/fallback, `font-feature-settings`, `letter-spacing`, `word-spacing`.
11. Scrolling: `overflow: hidden/scroll/**auto**`, scrollbars (overlay + classic), wheel/trackpad, `scroll-behavior`.
12. Hit testing (respecting transforms, clip, `pointer-events`) and a **minimal event dispatch** with capture/bubble.
13. macOS specifics: CoreText/`CTFont` enumeration + cache, `mmap`'d system fonts, **CJK fallback** (Blitz shipped a specific fix for CJK on Apple platforms), and Metal via wgpu.
14. A screenshot/diff harness from day 1 — visual regression is the only tractable correctness oracle at this scale.

**Explicitly not in 6 months:** JS, forms, iframes, tables (fake them with grid, as Blitz does, and be honest), `position: sticky`, 3D transforms, SVG *styling* (render only), container queries, multi-column, counters, `text-overflow` ellipsis (likely), blend modes.

### 12 months — "renders most sites; interactive but JS-poor"

Adds ~60–80k LOC. This is Blitz-beta-to-1.0 plus the beginning of scripting.

1. **JS via an embedded engine** — V8 (Aurora's choice after abandoning SpiderMonkey and Boa) or SpiderMonkey (Servo's choice). Do **not** write one. Budget 3–4 months just for bindings, GC integration and the event loop.
2. DOM bindings at useful breadth: `querySelector`, `innerHTML`/fragment parsing, `classList`/`style`/`dataset`, `addEventListener` with real propagation, `MutationObserver` (nothing works without it).
3. **Event loop**: task/microtask ordering, `setTimeout` **with nesting clamp**, `requestAnimationFrame`, hidden-tab throttling, `queueMicrotask`, promise job queue.
4. **Incremental style + layout**: dirty-bit propagation, invalidation sets, relayout boundaries. Retrofitting this is 5× harder than designing it in — do the seam early even if the first implementation is full relayout.
5. **Synchronous layout APIs**: `getBoundingClientRect`, `offsetWidth/Height`, `scrollTop/Left` — forced synchronous layout, correct and cheap.
6. **Tables for real**: fixed and auto table layout, anonymous table boxes, rowspan/colspan, border collapsing (contradiction resolution). Weeks, not days.
7. `position: sticky`, `text-overflow: ellipsis`, `line-clamp`, `color-scheme`, `content: counter()`, CSS counters.
8. **Forms** (basic): text inputs, checkboxes/radios, selects, buttons, focus ring, caret, IME composition events (Ladybird's Discord/Slate bug is the tell), `appearance`.
9. **Iframes / nested documents** (Blitz only got these in 0.3), same-origin at least, with a placeholder for cross-origin.
10. Compositing: layers for transforms/opacity/`position:fixed`, GPU scroll.
11. Accessibility via AccessKit, including positions/sizes (Servo only got this right in 2026).
12. **WPT + reftest CI on every PR** (Blitz only added this in 0.2; that was too late).

### 24 months — "a browser you could actually use"

Adds ~100–150k LOC, and this is where the project either becomes real or dies.

1. Shadow DOM + custom elements + slots — the single hardest DOM feature, and non-negotiable because every modern site uses it (Aurora's entire YouTube saga is this).
2. Cross-document navigation done properly: session history, `about:blank`, `window.open`, BFCache; Servo calls this "the gnarliest part of the HTML spec" and is still fixing it in 2026.
3. Multi-process or at minimum a hardened renderer boundary; a security model that survives renderer compromise (Ladybird's Trail-of-Bits backlog is the cautionary tale).
4. Bidi and vertical writing modes properly — `unicode-bidi`, inline-level `direction`, `writing-mode` for text.
5. Full form controls incl. validation, autofill, `::file-selector-button`, date/time pickers.
6. Media: `<video>`/`<audio>` via a real pipeline; canvas 2D; ideally WebGL2.
7. DevTools over CDP: element tree + picker, box model overlay, computed styles, console, network.
8. Storage (cookies, localStorage, IndexedDB), CORS, CSP, service workers (or explicit, documented refusal).
9. Performance work: parallel style/layout, display-list caching, `will-change`, `content-visibility`.
10. Crash reporting, task manager, profiles, extension points.

Realistically items 1–3 alone are a year.

### Ranking: top-1000-websites vs Speedometer 3 vs nice-to-have

**Tier A — needed for the top-1000 websites** (driven by Blitz's real-usage percentages):
`display` block/inline/inline-block · `position` static/relative/absolute/fixed · `overflow` incl. **`auto`** · `z-index` + stacking contexts · `box-sizing` · `width/height/min/max` · `margin`/`padding` + **margin collapsing** · `::before`/`::after` strings · `float`/`clear` · `border-radius` · `background-image/position/size/repeat` · `font-family/size/weight/style` + `@font-face` · `color`/`text-align`/`line-height` · `text-decoration`/`vertical-align`/`white-space` · `flexbox` · `transform` 2D · `opacity`/`visibility` · `box-shadow` · `transition`/`animation` · `list-style` · `cursor` · `pointer-events`/`user-select` · `text-overflow` · `line-clamp` · inline SVG *rendering* · `color-scheme` · `position: sticky` · basic tables · **Shadow DOM/custom elements** (as soon as there is JS) · **forms** (as soon as there is JS).

**Tier B — needed to score on Speedometer 3, largely orthogonal to Tier A:**
Small-tree block/flex layout (Speedometer pages are ~3–10k nodes; Taffy's own benchmark notes most popular sites are 3,000–10,000 nodes) · **fast DOM mutation** (create/insert/remove thousands of nodes) · **precise style invalidation + fast selector matching** · **fast JS↔DOM binding** (the actual bottleneck; V8/SpiderMonkey is not the problem) · `innerHTML` fragment parsing · `getBoundingClientRect`/`offsetHeight` forced-sync layout · event dispatch/bubbling · timers + microtask ordering · rAF · `classList`/`style` mutation.
Speedometer 3 deliberately contains **no tables, no floats, no SVG styling, no bidi, no forms, no media, no container queries**. Its workloads are demo to-do/editor/chart-like apps, scored as a geomean over subtests ([browserbench.org/Speedometer3.0](https://browserbench.org/Speedometer3.0/); 3.1 supersedes it and adds a custom-CSS workload). **Optimising for Speedometer will not make real sites render, and making real sites render will not move Speedometer.** Ladybird's September — +50% Speedometer 3 in one month — came from timer clamping, hidden-tab throttling, offscreen rAF suppression and skipping unchanged frames. That is event-loop hygiene, not layout.

**Tier C — nice to have (defer past 24 months):** 3D transforms/`perspective` · `mix-blend-mode`/`backdrop-filter` · multi-column · anchor positioning · scroll-state queries · `@page`/print · `mask-composite` · `corner-shape` · vertical writing modes · WebGL/WebGPU/WebXR/WebRTC · full canvas 2D · extension APIs · PWA/service workers · password manager · sync.

---

## 7. Top 10 things these projects got wrong or underestimated

Ordered by how expensive the mistake was to fix late.

**1. Inline/text layout — the mountain is not box layout.**
Everyone assumes "use Parley/rustybuzz for text, Taffy for boxes, done." Taffy's README is explicit that it implements **Block, Flexbox and CSS Grid** and does not do text; its own benchmark note says sites "require text layout, which neither yoga nor taffy implement." Blitz spent three release cycles on white-space collapsing, `vertical-align`, inline-span backgrounds, `text-decoration-thickness/inset/offset/position`. Servo needed a dedicated fix for **shaping text across inline boxes** in 2026. The line-breaking-to-box algorithm, inline box fragmentation across lines, and bidi reordering are yours to write, and they touch everything downstream.

**2. Tables.**
Blitz *emulates table layout with CSS Grid* and lists real table layout as a 1.0 item; its `border-collapse` assumes uniform borders and takes styles from the first cell. Servo has a dedicated table-spec expert as a maintainer and still fixes table bugs. Fixed + auto table layout, anonymous table box generation, rowspan/colspan spanning, and border-conflict resolution is a multi-week sub-project that interacts with intrinsic sizing everywhere.

**3. Floats, margin collapsing and BFC — the coupling nobody plans for.**
Blitz landed floats only in the 0.3 cycle (**PR #265**), roughly three years in. Floats are not a feature; they are a constraint system that intersects inline layout (line boxes shorten around floats), clearance, BFC establishment, and margin collapsing. Margin collapsing has four distinct cases and is the source of most "why is there a phantom gap" bugs. Get containing-block chains, out-of-flow positioning and BFC right *together* or you will rewrite layout twice.

**4. Incremental layout and invalidation — the thing you cannot retrofit.**
Full relayout per frame is easy and gets you a demo. Correct dirty-root computation is the hardest part of a real engine and the main Speedometer lever. Servo — fourteen years in — is still fixing "many edge cases around determining dirty roots for incremental layout" (#46906). Blitz only got incremental box-tree construction in 0.2. Design the invalidation seam in month one even if the first implementation is a full relayout.

**5. Stacking contexts and paint order.**
The paint-order algorithm (backgrounds → floats → inline → positioned descendants → stacking contexts, with context-creating properties like `opacity`, `transform`, `filter`, `isolation`, `will-change`) gets reimplemented three times by every engine. Blitz shipped "support for non-local z-index" as a 0.3 milestone and admits nodes with `opacity` are "currently clipped regardless of the overflow style." Cheap to get 90% right, expensive to get the last 10%.

**6. GC and DOM cycles — Rust does not save you.**
This is the most expensive wrong assumption available to a Rust team. The DOM+JS object graph is *inherently* cyclic; ownership cannot express it. Servo is supporting "a large scale rewrite of Servo's JS engine integration to address intermittent panics related to garbage collection" and is still adding *static prevention* for a footgun that produces uncollectible GC cycles. Ladybird leaked **11 GB in 4 hours** and **15 GB in 20 hours** because replaced documents were never collected. You need a real tracing GC or cycle collector for the DOM, designed before you write bindings. Pick: (a) SpiderMonkey's GC with `Rooted`/`Handle` discipline, (b) V8's with persistent handles + weak callbacks, or (c) your own arena + cycle collector with JS-visible identity. Whichever you choose, it constrains every API in the engine.

**7. Event-loop semantics and document lifecycle — the "why is our browser at 100% CPU / 15 GB" cluster.**
Spec-mandated timer nesting clamp (4 ms after 5 levels), hidden-tab throttling, rAF suppression for offscreen content, frame-skip when nothing changed, and the lifecycle of replaced documents. Ladybird's entire September performance win was this, not layout. Nothing in a "render HTML/CSS" mental model hints that these exist, and they dominate real-world perceived quality and battery.

**8. Synchronous layout APIs and hit testing.**
`getBoundingClientRect`, `offsetWidth`, `scrollTop` force layout *now*, and get called in loops by every framework. Hit testing must compose transforms, clips, `pointer-events`, z-order and scroll offsets — and any inaccuracy produces "clicks land on the wrong thing," the most user-visible bug class there is. Blitz had to refactor event handling wholesale and only recently added `pointer-events`.

**9. "Complete" quietly means forms, iframes, shadow DOM, media and a security model.**
The brief says "complete usable engine." Concretely that pulls in: form controls + IME + caret + validation; multi-document iframes and navigation (Servo: "the gnarliest parts of the HTML spec"; Blitz only got nested documents and `<iframe>` in 0.3, four years in); Shadow DOM + custom elements + slots; `<video>`; and a process/permission boundary that survives renderer compromise (Ladybird's Trail-of-Bits audit produced "a considerable security backlog" *and crafted full exploit chains* — at year seven, pre-alpha). Each of these is a project, not a feature.

**10. Spec-test volume, harness cost, and the absence of DevTools.**
Ladybird: **2,109,072 passing WPT subtests** after seven years, gaining ~20k/month. Blitz only put WPT on every PR in 0.2 (Oct 2025) — roughly 20 months in — and its CSS subsuite is still climbing. The *harness* is itself a multi-month build: reftest infrastructure, crashtests, per-directory expectation files, flaky-test triage, and a CI matrix. And without CDP DevTools (a Blitz 1.0 item) you cannot debug your own engine on a real page. Underestimate the verification infrastructure and you spend year two coding blind.

**Bonus, and it belongs in the top 3 for a Rust project specifically:** **Stylo is not a drop-in.** It is published (v0.22, 444k downloads) but requires you to implement Servo's DOM traits; Blitz carries a dedicated `stylo_taffy` crate just to glue Stylo to Taffy, triple-licensed to ease Servo interop. Aurora hit a Stylo panic when an element had no computed style. Budget real weeks for the Stylo integration and expect to fork or pin.

---

## 8. Comparison table

| Project | Language | Age | LOC | Covers | Shell? | License | Lesson |
|---|---|---|---|---|---|---|---|
| **Servo** | Rust | 14.7 yrs (2012-02) | ~544k Rust | HTML/CSS/JS (SpiderMonkey), CSS Grid (2026), flexbox, tables, iframes, WebGL/WebGPU, a11y, DevTools, multiprocess | **Yes** — servoshell/minibrowser, nightly macOS arm64, unsigned | MPL-2.0 | 30 maintainers + 1 part-time funded reviewer; "don't log into your bank" after 14 years. Crates are the real gift |
| **Blitz** | Rust | 4.7 yrs repo / ~2.5 serious | **~68–80k Rust** | HTML+CSS only. Block/inline/flex/grid/floats/tables-via-grid/2D transforms/animations. **No JS** | Yes — Browser UI for Win/macOS/Linux/Android | Apache-2.0 OR MIT (`stylo_taffy` +MPL-2.0) | Max reuse of Stylo+Taffy+Parley+Vello → beta in 2.5 yrs, still no `overflow:auto`, sticky, or real tables |
| **Aurora** | Rust | 7 months (2026-03) | ~28.6k Rust | Integration layer: V8 + blitz-dom/Stylo + blitz-paint/Vello + reqwest | Prototype window + PNG | MPL-2.0 | 6 months gets a prototype that paints, not an engine. Two DOM trees = top technical debt |
| **Ladybird** | C++/Rust | **7.3 yrs** (2019-06) | **~1.19M engine** (repo ~1.8M code + 774k HTML tests) | Full browser: LibWeb+LibJS, grid, subgrid, container queries, shadow DOM, sandboxing, WebGL, media, DevTools | Yes — Qt, macOS/Linux, **alpha still pending** | BSD-2-Clause | 1.2M LOC + funded team + 7 yrs → Speedometer 3 ≈ **5.9**, alpha unshipped. This is the true cost |
| **verso** | Rust | ~2 yrs | small | Servo-based shell, multi-view/multi-window | Yes (on Servo) | — | Building the *shell* is its own project; defers to servoshell |
| **Lexbor** | C | — | — | HTML/CSS parse + DOM + serialize (3.0.1 in Debian) | No | Apache-2.0 | Parsing is a commodity; never write it |
| **Ultralight** | C++ | 2.0 | — | "Pure-GPU HTML renderer", commercial | Minimal | Proprietary | Commercial non-browser renderer is viable only by scoping HTML/CSS down |
| **Nyx** | — | — | — | **Unverified** | — | — | No primary source found; do not cite |

---

## 9. Recommendation for this project

1. **Do not scope v0.1 as "complete usable engine" at 6 months.** Reframe: 6 months = *static-content engine with a visual-regression corpus*; 12 months = *plus JS + forms + tables + iframes*; 24 months = *plus shadow DOM, navigation, sandboxing, DevTools*.
2. **Reuse aggressively at the commodity layer** — `html5ever`, `cssparser`, `selectors`, `rustybuzz` + `skrifa`/`fontique`, `vello_cpu`/`vello` on wgpu/Metal, `accesskit`, `winit`. **Do not** plan on `webrender` (391k downloads for a reason) or expect `stylo` to be drop-in.
3. **Write the four things that are actually yours:** the DOM object graph and its GC, box-tree construction + inline layout + line breaking, the invalidation/incremental-layout seam, and the event loop. These are where the 1.2M lines live.
4. **Commit to a cycle-collecting DOM design before writing bindings.** This is the decision that is cheapest now and most expensive later.
5. **Stand up WPT + reftest + visual-diff CI in month one**, not month twenty.
6. **Pick the target sites and publish the pass list.** Blitz's `% use` status page and NLnet's "Wikipedia, news site, search engine" scope are the right level of honesty.
7. **The realistic competitive claim is not "renders the web like Safari."** It is "renders a defined corpus of content sites correctly, fast, on Apple Silicon, with a codebase a small team can hold in its head." That is achievable. "Complete usable engine" at 6 months is not.

---

## Sources

**Servo**
- [servo.org/blog/2026/10/09/august-in-servo](https://servo.org/blog/2026/10/09/august-in-servo/) — 417 commits, CSS Grid, SpiderMonkey 153, text selection, threadboost +50% Speedometer, $8,128/mo
- [servo.org/blog/2026/09/15/one-year-of-sponsorship](https://servo.org/blog/2026/09/15/one-year-of-sponsorship/) — 1,150 PRs reviewed, 8 maintainers nominated
- [servo.org/download](https://servo.org/download/) — macOS arm64 nightly, unsigned, "don't log into your bank"
- [servo.org/wpt](https://servo.org/wpt/), [wpt.fyi servo results](https://wpt.fyi/results/?product=servo)
- [github.com/servo/servo](https://github.com/servo/servo) — repo metadata, 58,888 commits, ~544k Rust LOC
- [github.com/servo/servo/tree/main/components](https://github.com/servo/servo/tree/main/components) — architecture
- [github.com/servo/project/blob/main/governance/MAINTAINERS.md](https://github.com/servo/project/blob/main/governance/MAINTAINERS.md) — 30 maintainers
- [book.servo.org](https://book.servo.org) — constellation / compositor / script
- [nlnet.nl/project/Servo-Navigation_Downloads](https://nlnet.nl/project/Servo-Navigation_Downloads/)
- [phoronix.com/news/Servo-0.6-Changes](https://www.phoronix.com/news/Servo-0.6-Changes)

**Blitz**
- [github.com/DioxusLabs/blitz](https://github.com/DioxusLabs/blitz) — README, architecture, licence, repo metadata, 2,126 commits
- [blitz.is/status/css](https://blitz.is/status/css) — per-property status with real-world usage percentages
- [github.com/DioxusLabs/blitz/issues/119](https://github.com/DioxusLabs/blitz/issues/119) — roadmap 0.1/0.2/0.3/1.0
- [nlnet.nl/project/Blitz](https://nlnet.nl/project/Blitz/) — NGI Zero Core grant from 2024-12, scope, no-JS
- [github.com/DioxusLabs/taffy](https://github.com/DioxusLabs/taffy) — block/flex/grid only, no text layout, 3,000–10,000 node note

**Aurora**
- [github.com/JohannaWeb/Aurora](https://github.com/JohannaWeb/Aurora) — README (V8, not SpiderMonkey; blitz-dom integration; known limits), created 2026-03-30, 161 commits, ~28.6k Rust LOC
- [crates.io/crates/aurora-engine](https://crates.io/crates/aurora-engine)

**Ladybird**
- [ladybird.org](https://ladybird.org/) — alpha "later this year", small full-time team, 18-month runway, sponsor tiers
- [ladybird.org/newsletter/2026-09-30](https://ladybird.org/newsletter/2026-09-30) — Trail of Bits, Speedometer 3 ≈ 5.9, WPT 2,109,072, 11/15 GB leaks, timer clamp, cross-site iframes
- [ladybird.org/posts/adopting-rust](https://ladybird.org/posts/adopting-rust/) — Rust port with AI assistance
- [ladybird.org/posts/changing-how-we-develop-ladybird](https://ladybird.org/posts/changing-how-we-develop-ladybird/) — closed to public PRs, 2026-06-05
- [en.wikipedia.org/wiki/Ladybird_(web_browser)](https://en.wikipedia.org/wiki/Ladybird_(web_browser)) — timeline, funding, WPT ranking, 2026/2027/2028 plan
- [w3.org TPAC 2024 breakouts](https://www.w3.org/2024/09/TPAC/breakouts.html) — June 2019 LibHTML first commit
- [W3C talk: Ladybird, an independent engine](https://www.w3.org/events/meetings/8e1ca708-fdbf-4264-a79b-4c953fa85248/) — alpha/beta/stable plan
- [heise.de — $1M from Chris Wanstrath](https://www.heise.de/en/article/Ladybird-web-browser-takes-off-One-million-US-dollars-from-GitHub-founder-9789840.html)

**Others / crates**
- [github.com/tauri-apps/verso](https://github.com/tauri-apps/verso)
- [ultralig.ht](https://ultralig.ht/), [Ultralight 2.0 changelog](https://ultralig.ht/docs/2.0/changelog)
- [packages.debian.org/forky/liblexbor-dev](https://packages.debian.org/forky/liblexbor-dev)
- [github.com/harfbuzz/rustybuzz](https://github.com/harfbuzz/rustybuzz)
- [browserbench.org/Speedometer3.0](https://browserbench.org/Speedometer3.0/), [Speedometer 3.1](https://browserbench.org/Speedometer3.1)
- crates.io API v1 for download counts and versions, queried 2026-10-25
- GitHub REST API for repo metadata, commit counts, participation stats, queried 2026-10-25
- Line counts from shallow/sparse clones of `DioxusLabs/blitz` and `LadybirdBrowser/ladybird`, 2026-10-25
