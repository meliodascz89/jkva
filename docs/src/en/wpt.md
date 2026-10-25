# Web Platform Tests (reftests)

JKva runs the subset of WPT that needs no JavaScript: **reftests**. A reftest is a pair of pages — the test and a reference named by `<link rel="match">` — that must render identically (or, for `mismatch`, differently).

## Method

- A sparse WPT checkout: `css/css-color` only.
- Both pages rendered with `jkva-shot` at 800×600.
- Exact pixel comparison, no tolerance.
- A page that needs an external stylesheet, a script, an image, an iframe, media, `@import` or `url()` is reported as **unsupported**, never as a pass: otherwise both sides would lose the same resource and compare equal for the wrong reason.

```bash
git clone --depth 1 --filter=blob:none --sparse https://github.com/web-platform-tests/wpt /tmp/wpt
cd /tmp/wpt && git sparse-checkout set css/css-color
cd - && JKVA_SHOT=target/debug/jkva-shot \
  cargo run -p wpt-reftest-runner -- /tmp/wpt/css/css-color --limit 25
```

## 2026-10-25 — first run, 25 reftests

| Result | Count |
|---|---|
| pass | 10 |
| fail | 7 |
| unsupported | 7 |
| error | 1 |

## What this number is not

It is **not** a WPT score and not a conformance claim.

- A pass can be a **shared failure**: if both the test and the reference use a feature the engine does not implement yet, both render the same and the pair "passes". The harness removes the most obvious cause (external dependencies), not all of them.
- One error came from a reference outside the sparse checkout (`/css/reference/...`), not from rendering.
- The official `wpt run` harness, with its product adapter and WebDriver, arrives with the JS engine in M4.

The honest reading: the binary works, the corpus runner works, and the number is a floor with a known upward bias. It will be replaced by the official harness, not polished.
