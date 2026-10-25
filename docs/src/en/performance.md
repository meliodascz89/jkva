# Performance baseline

Every number here names the fixture, the iteration count, the machine and the command. A number without those is not a measurement.

## 2026-10-25 — M1 baseline

- Machine: Apple M4, macOS 27.0.1, rustc 1.99.1 (stable)
- Fixture: 200 rows (602 DOM nodes), 200 CSS rules, 16,221 B HTML, 9,413 B CSS
- Iterations: 20, median reported
- Command: `cargo run -p bench-runner`

| Bench | Median |
|---|---|
| parse-html | 2.46 ms |
| parse-css | 0.80 ms |
| cascade | 2.60 ms |
| shape-text | 30.5 µs |

## What these numbers mean

They are layer micro-benchmarks, not Speedometer and not a comparison with Chrome.

The cascade used to be the slow one at **19.93 ms** because every rule was tested against every element. Rule indexing by id, class and type (rightmost compound, with a universal fallback) brought it to 2.60 ms on the same fixture — 8x in debug and 2.2x in release. What is left is per-element allocation, not matching; that is the next measured step.

## Rules

- Re-run the same fixture when a change touches parsing, style or text.
- Never quote a number without the fixture size and the machine.
- Publish regressions as loudly as improvements.
