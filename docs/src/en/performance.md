# Performance baseline

Every number here names the fixture, the iteration count, the machine and the command. A number without those is not a measurement.

## 2026-10-25 — M1 baseline

- Machine: Apple M4, macOS 27.0.1, rustc 1.99.1 (stable)
- Fixture: 200 rows (602 DOM nodes), 200 CSS rules, 16,221 B HTML, 9,413 B CSS
- Iterations: 20, median reported
- Command: `cargo run -p bench-runner`

| Bench | Median |
|---|---|
| parse-html | 2.73 ms |
| parse-css | 0.80 ms |
| cascade | 1.94 ms |
| shape-text | 31.8 µs |

## What these numbers mean

They are layer micro-benchmarks, not Speedometer and not a comparison with Chrome.

The cascade used to be the slow one at **19.93 ms** because every rule was tested against every element. Two measured steps fixed it: rule indexing by id, class and type (rightmost compound, universal fallback), then removing two thirds of the per-element allocations. It is now **1.94 ms** on the same fixture — 10.3x in debug and 4.8x in release since the first baseline. What is left is one `String` per computed field (23 per element) and that is the documented floor until `ComputedStyle` stops owning text.

## Rules

- Re-run the same fixture when a change touches parsing, style or text.
- Never quote a number without the fixture size and the machine.
- Publish regressions as loudly as improvements.
