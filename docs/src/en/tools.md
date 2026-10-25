# Tools

`tools/` holds two small command-line programs that turn "does this still work?" into a number a machine can check: `reftest-diff` for reference tests and `wpt-report` for Web Platform Tests results. Neither tool links against the engine or touches Metal, so both build and run on Linux as well as on Apple Silicon.

## One convention, two jobs

Both tools follow the same exit-code convention, and that convention is what makes them usable in a gate at all:

| Exit code | Meaning |
|---|---|
| `0` | The thing you measured is within tolerance. |
| `1` | The tool did its job and the answer is no: the images differ, or the pass rate is below the target. |
| `2` | The tool could not do its job: bad arguments, unreadable file, malformed input. |

Keeping `1` and `2` apart is the point. A red build can then say "the page changed" or "the tool broke", and those two failures need different people to look at them.

## Running them

For a one-off look, let Cargo build and run:

```bash
cargo run -p reftest-diff -- expected.png actual.png
cargo run -p wpt-report -- report.json
```

The `--` separates Cargo's arguments from the program's. In CI, build once and call the binaries directly:

```bash
cargo build --release -p reftest-diff -p wpt-report
./target/release/reftest-diff expected.png actual.png
```

## reftest-diff

A reference test compares a screenshot rendered by the engine against an expected image. `reftest-diff` answers the only question that matters about the pair: how many pixels actually differ, and by how much.

```text
reftest-diff <expected.png> <actual.png> [--threshold N] [--max-ratio R] [--out diff.png]
```

| Option | Default | Meaning |
|---|---|---|
| `--threshold N` | `0` | Per-channel tolerance in the range `0`–`255`. A pixel counts as different when its largest channel delta is **greater than** `N`, so `--threshold 4` still accepts a delta of exactly 4. |
| `--max-ratio R` | `0.0` | Fraction of differing pixels still accepted. `--max-ratio 0.01` passes a pair in which up to 1% of the pixels differ. |
| `--out diff.png` | nothing written | Write a diff image: the actual screenshot with every differing pixel painted red. |

The comparison is deliberately literal. There is no perceptual metric, no anti-aliasing heuristic and no "close enough" fuzz: once a pixel's largest channel delta exceeds the threshold, that pixel differs. That bluntness is the feature, because it makes the result reproducible when the expected image was rendered on a different machine.

Here is a real run. The actual screenshot has a 50 × 50 block painted the wrong colour:

```console
$ reftest-diff expected/hello.png actual/hello.png --max-ratio 0.001 --out hello-diff.png
diff image: hello-diff.png
800x600: 2500/480000 pixels differ (0.5208%), max delta 240, threshold 0
$ echo $?
1
```

0.52% of the image differs and the caller allowed 0.1%, so the run exits `1`. Without `--max-ratio` it also exits `1`, because the default tolerance is zero differing pixels. Against an identical image it prints `0/480000 pixels differ (0.0000%), max delta 0` and exits `0`.

Three things the usage line does not say:

- **Both images must have the same dimensions.** A size mismatch is reported on stderr and exits `1`, not `2`. Two different sizes are a difference in the pair, not a broken tool.
- **Only 8-bit PNGs are accepted** — RGB, RGBA, grayscale, or grayscale with alpha; palette images are expanded first. A 16-bit reference image is refused with exit `2`.
- **There is no `--help` flag.** Running the tool with no arguments, or with an unknown flag, prints the usage line to stderr and exits `2`.

## wpt-report

The [Web Platform Tests](https://web-platform-tests.org/) runner writes a JSON report with `--log-wptreport`. `wpt-report` turns that file into a table you can read in a code review: how many subtests passed, per top-level test directory and overall.

```text
wpt-report <report.json> [--min-pass-rate R] [--top N]
```

| Option | Default | Meaning |
|---|---|---|
| `--min-pass-rate R` | `0.0` | Lowest acceptable pass rate, as a **fraction** between `0` and `1`. `--min-pass-rate 0.95` means 95%. |
| `--top N` | `12` | How many directory rows to print. The `TOTAL` row is always printed. |

A run over a small report looks like this:

```console
$ wpt-report report.json
directory        passed   failed  skipped pass rate
css                  13        3        0    81.25%
dom                  10        1        0    90.91%
infrastructure        2        1        0    66.67%
html                  3        0        1   100.00%
TOTAL                28        5        1    84.85%
$ echo $?
0
```

Directories are sorted by failure count, highest first, and ties are ordered by name — so the top of the table is where the work is. Ask for a gate and the same report fails:

```console
$ wpt-report report.json --min-pass-rate 0.95
wpt-report: pass rate 84.85% is below the target 95.00%
$ echo $?
1
```

A failed gate is still a report, which is why the table goes to stdout either way and only the verdict goes to stderr.

The counting rules are worth knowing before you argue with the number:

- A test that reports subtests is counted **per subtest**; a test without subtests is counted **once**, from its own status.
- `PASS` and `OK` count as passed. `FAIL`, `ERROR`, `TIMEOUT`, `CRASH` and `PRECONDITION_FAILED` count as failed. `SKIP` counts as skipped.
- **Any unrecognised status counts as a failure**, so a runner that invents a new status cannot quietly raise the pass rate.
- The pass rate is `passed / (passed + failed)`. Skipped subtests appear in the table but are excluded from the rate, because a test that never ran says nothing about the engine. If nothing ran, the rate is `0.00%` rather than `NaN`.

Malformed input ends the run with exit `2` and one line on stderr: a missing file, invalid JSON, or a document with no `results` array.

## Not a gate yet

Both tools are written to be gates and both are compiled by the Linux CI job; their unit tests run as part of `cargo test --workspace` on macOS. Neither is wired into a workflow that can fail a pull request, because there is nothing to measure yet: the repository has no reference-image corpus to diff and no WPT run to summarise. ADR-023 lists the reference-test diff and the WPT runner among the CI plugins, so treat the exit codes above as the contract these tools are being built to — not as a promise about what `main` enforces today.

## reftest-runner

Renders every `*.html` in a corpus directory with `jkva-shot` and compares the
result to `expected/<name>.png`.

```bash
cargo run -p jkva-shell --bin jkva-shot -- examples/hello.html examples/expected/hello.png
cargo run -p reftest-runner -- examples          # ok / FAIL per page
cargo run -p reftest-runner -- examples --update # refresh the golden images
```

Exit codes: `0` all match, `1` differences or missing images, `2` error. Set
`JKVA_SHOT` to use a different renderer binary; the default is
`target/debug/jkva-shot`.
