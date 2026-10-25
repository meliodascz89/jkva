## Stav na konci dne (2026-10-25)

- Poslední kódový commit: `f3997d3` (`align-self`), CI zelené; tahle aktualizace dokumentace jde po něm.
- M0 a M1 jsou uzavřené. M2 je rozpracované.
- Pracovní strom je čistý. Rozpracovaný `flex-wrap`/`grow` byl zastaven a vrácen, místo aby zůstal napůl hotový; další vývojář startuje ze zeleného stromu.
- Další krok: flex wrap plus `flex-grow`/`flex-shrink`/`flex-basis` (čtyři vlastnosti v kaskádě a zalamování řádků v `layout_flex_content`), pak grid a tabulky.

# Stav milníků

Co je hotové, co je ověřené a co se záměrně ještě nedělá. Každý řádek ukazuje na kód nebo dokument, který si můžeš otevřít.

## M0 — základy

| Položka | Stav | Důkaz |
|---|---|---|
| Repo, licence, komunitní soubory | hotovo | `LICENSE-*`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md` |
| CI: formát, clippy, testy, rustdoc, dokumentace | hotovo | `.github/workflows/ci.yml`, zelené u každého pushe |
| Kostra dokumentace EN + CS | hotovo | `docs/`, tato kniha |
| Architektonická rozhodnutí | hotovo | [`briefing/DECISIONS.md`](https://github.com/meliodascz89/jkva/blob/main/briefing/DECISIONS.md) |
| Walking skeleton: Metal zařízení a trojúhelník | hotovo | `crates/jkva-gfx`, `gfx-clear`, `gfx-triangle` |
| IPC kontrakty | hotovo | `crates/jkva-ipc`: verzované rámce s délkou; kontroly verze, velikosti a zkrácení |
| Harness: pixel diff, corpus runner, WPT report, WPT reftesty, benchmarky | hotovo | `tools/reftest-diff`, `tools/reftest-runner`, `tools/wpt-report`, `tools/wpt-reftest-runner`, `tools/bench-runner` |
| Veřejný repozitář od prvního commitu | hotovo | <https://github.com/meliodascz89/jkva> |

## M1 — první vykreslená stránka

| Položka | Stav | Důkaz |
|---|---|---|
| HTML → DOM | hotovo | `jkva-dom`, html5ever 0.40; 38 testů |
| CSS parser, matching, kaskáda, UA sheet | hotovo | `jkva-style`; 157 testů |
| Blokový a inline layout | hotovo | `jkva-layout`; 73 testů včetně flexu; inline boxy, procenta, auto centrování, zalamování |
| Shaping a rasterizace glyfů | hotovo | `jkva-text`, fontdb + harfrust + swash; 21 testů |
| Raster a kompozice | hotovo | `jkva-gfx` scéna, alpha blending |
| Screenshot harness se zlatými stránkami | hotovo | `examples/hello.html`, `examples/cards.html`, `tools/reftest-runner` |
| WPT reftest harness | hotovo | `tools/wpt-reftest-runner`; první běh 10 pass / 7 fail / 7 unsupported z 25 |
| Inkrementální seam | hotovo | `jkva-shell::Frame` vrací `DamageRegion`; layout je zatím celodokumentový |
| Dokumentace a ADR aktuální | hotovo | tato stránka, `docs/`, `CHANGELOG.md` |

## M2 — flex (podmnožina, rozpracováno)

| Položka | Stav | Důkaz |
|---|---|---|
| Flex formátovací kontext | hotovo | `layout_flex_content`; row/column, 5 justify režimů, 4 align režimy, gap |
| Flex dostupný z enginu | hotovo | shell mapuje `display: flex` na `Display::Flex` |
| `align-self` na itemu | hotovo | fallback na `align-items`; testováno |
| Korpusová stránka se zlatým obrázkem | hotovo | `examples/flex.html`, `examples/expected/flex.png` |
| Wrap, grow/shrink/basis, order, align-self | zatím ne | další krok M2 |
| Grid, tabulky, floaty, obrázky, scroll, hit testing, vrstvy | zatím ne | M2 |

## Známé mezery (poctivý seznam)

Nejsou schované; jsou to úkoly M2+.

- **Oficiální harness `wpt run`** potřebuje JS engine a WebDriver, tedy M4. Reftest runner je náhrada pro M1, s publikovanými limity.
- **Layout je celodokumentový průchod.** Inkrementální je dnes jen damage.
- **Cena kaskády** je 1,94 ms na fixture se 602 uzly a je v [Performance baseline](performance.md).

## Další

Flex wrap, grow/shrink a basis, pak grid, tabulky, floaty, obrázky, scroll, hit-testing, vrstvy a stacking contexts. Tabulka milníků je v záznamu rozhodnutí.
