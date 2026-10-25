# Contributing

The short version is in [`CONTRIBUTING.md`](https://github.com/meliodascz89/jkva/blob/main/CONTRIBUTING.md). This page adds the rules that are specific to the engine.

## Changing a decision

Architecture decisions are recorded in [`briefing/DECISIONS.md`](https://github.com/meliodascz89/jkva/blob/main/briefing/DECISIONS.md). If a change contradicts one of them, write a new ADR in `decisions/` first and discuss it in the pull request. The code follows the ADR, never the other way around.

## Tests that count

- A parser fix needs the smallest HTML/CSS input that reproduces the bug.
- A layout fix needs a reference screenshot or a numeric assertion, not "it looks right".
- A performance change needs a before/after number from `benches/`, on the machine named in the report.
- A memory change needs a `footprint` number if it touches caches or arenas.

## Documentation

Public items are documented in English. User-facing guides exist in English and Czech; when you change behaviour, update both or the Czech page will silently rot. The Czech page is allowed to be shorter, but it must not contradict the English one.

This book is built with `mdbook build docs`; the `docs` CI job runs the same command. Be aware that mdBook does not check whether a link points at a page that exists. A typo builds cleanly and 404s in the browser, so follow the links yourself before you push.
