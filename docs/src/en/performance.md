# Performance baseline

Every number here names the fixture, the iteration count, the machine and the command. A number without those is not a measurement.

## 2026-10-25 — M1 baseline

- Machine: Apple M4, macOS 27.0.1, rustc 1.99.1 (stable)
- Fixture: 200 rows (602 DOM nodes), 200 CSS rules, 16,221 B HTML, 9,413 B CSS
- Iterations: 20, median reported
- Command: `cargo run -p bench-runner`

| Bench | Median |
|---|---|
| parse-html | 3.02 ms |
| parse-css | 0.82 ms |
| cascade | 19.93 ms |
| shape-text | 33.8 µs |

## What these numbers mean

They are layer micro-benchmarks, not Speedometer and not a comparison with Chrome. `cascade` is the slow one: styles are matched by scanning every rule for every element, with no rule index or style sharing yet. That is a known M2 task, and this table exists to prove the improvement when it lands.

## Rules

- Re-run the same fixture when a change touches parsing, style or text.
- Never quote a number without the fixture size and the machine.
- Publish regressions as loudly as improvements.
