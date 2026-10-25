[English](README.md) | **Čeština**

[![ci](https://github.com/meliodascz89/jkva/actions/workflows/ci.yml/badge.svg)](https://github.com/meliodascz89/jkva/actions/workflows/ci.yml)

# JKva

> Browser engine psaný od nuly v Rustu. Apple Silicon first, megabajty last.

JKva je raná fáze browser engine: vlastní DOM, stylovací systém, layout, rasterizér, kompozitor i event loop — žádná zděděná dvacet let stará C++ vrstva, žádný fork Chromia.

**Stav: M1 — první stránka se vykresluje.** Metal clear, MSL trojúhelník a obdélníky z display listu fungují, HTML se parsuje přes html5ever, CSS se parsuje a kaskáda počítá computed styles. Layout zatím není a neexistuje cesta z HTML souboru k pixelům; první vykreslená stránka je milník M1.

![The first page rendered by JKva](docs/images/first-page.png)

*První stránka vykreslená JKvou od začátku do konce: náš DOM, naše kaskáda, náš blokový layout, náš text a Metal. `examples/hello.html`, 800×600.*

## Co v repozitáři je

| Cesta | Co to je |
|---|---|
| `crates/` | engine, jeden crate na vrstvu (`jkva-core`, `jkva-dom`, `jkva-style`, `jkva-layout`, `jkva-text`, `jkva-paint`, `jkva-gfx`, `jkva-js`, `jkva-api`, `jkva-engine`, `jkva-platform`, `jkva-shell`) |
| `briefing/` | 99 rozhodnutí, na kterých stojí architektura, odpovědi a z nich odvozené ADR |
| `research/` | ověřené podklady o crate ekosystému 2026, benchmarcích a prior artu |
| `docs/` | dokumentace v angličtině a češtině (mdBook) |
| `decisions/` | odkaz na záznam rozhodnutí |

## Sestavení

```bash
# vyžaduje Rust stable (edition 2024) a macOS na Apple Silicon
cargo build --workspace
cargo run -p jkva-platform --bin metal-probe   # vypíše Metal zařízení
cargo test --workspace
```

Toolchain, cíle a CI jsou popsané v [`docs/src/cs/building.md`](docs/src/cs/building.md).

## Kde jsou rozhodnutí

- [`briefing/DECISIONS.md`](briefing/DECISIONS.md) — 24 ADR: rozsah, architektura, měření, tým.
- [`briefing/BRIEFING-0.1.cs.md`](briefing/BRIEFING-0.1.cs.md) — 99 otázek a zvolené odpovědi.
- [`briefing/ANSWERS.md`](briefing/ANSWERS.md) — odpovědi včetně odchylek od doporučení.

## Licence

MIT OR Apache-2.0, podle tvé volby. Viz [`LICENSE-MIT`](LICENSE-MIT) a [`LICENSE-APACHE`](LICENSE-APACHE).
