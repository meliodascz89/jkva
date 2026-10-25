**English** | [Čeština](../cs/index.md)

# JKva

JKva is a browser engine written from scratch in Rust for Apple Silicon. It is not a Chromium fork and it does not wrap an existing engine.

The project is at **M0 (foundations)**. The repository, CI and the platform probe exist; the first rendered page arrives in M1.

## Principles

1. **Own the engine, buy the standards.** Parsers, text shaping, the JS engine and the GPU API are dependencies. The DOM, cascade, layout, paint, compositor and event loop are ours.
2. **Measure, then claim.** Every performance statement is published with its method and the machine it ran on.
3. **Decide in public.** Architecture decisions are ADRs, not hallway conversations.
4. **Memory is a feature.** Arenas, budgets and a footprint metric from day one.

## Start here

- [Architecture](architecture.md) — what each crate owns.
- [Building](building.md)
- [Performance baseline](performance.md)
- [Web Platform Tests](wpt.md) — how to compile and run the probe.
- [Tools](tools.md) — the two command-line programs for reference tests and WPT results.
- [Contributing](contributing.md) — how to send a change.
- [`briefing/DECISIONS.md`](https://github.com/meliodascz89/jkva/blob/main/briefing/DECISIONS.md) — the 24 ADRs behind all of it.
