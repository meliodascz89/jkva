# Contributing to JKva

Thanks for taking a look. JKva is early, so the most useful contributions right now are bug reports, failing test cases from real pages, and documentation fixes.

## Before you start

- Read [`briefing/DECISIONS.md`](briefing/DECISIONS.md). The architecture is decided; changes to those decisions need a new ADR.
- For anything larger than a typo, open an issue first. It is cheaper than reviewing a large pull request that goes the wrong way.
- By contributing you agree to license your work under MIT OR Apache-2.0.

## Build and test

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Commits

Conventional Commits, with a body that answers four questions:

```
feat(dom): parse <template> into a document fragment

Co: what changed
Proč: why it was needed
Dopad: what it affects (crates, behaviour, benchmarks)
Měření: numbers, if the change is about performance
```

`feat`, `fix`, `docs`, `test`, `perf`, `refactor`, `build`, `ci`, `chore`.

## Pull requests

- One concern per PR. Small PRs get reviewed faster.
- Every public item needs documentation; `missing_docs` is denied at compile time.
- Unsafe code needs a `// SAFETY:` comment explaining the invariant.
- Behaviour changes need a test. Parser and layout fixes need a case that fails before and passes after.
- Performance claims need a number and the machine it was measured on.

## Code of conduct

Be decent. See [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
