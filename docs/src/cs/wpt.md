# Web Platform Tests (reftesty)

JKva spouští tu část WPT, která nepotřebuje JavaScript: **reftesty**. Reftest je dvojice stránek — test a reference určená `<link rel="match">` — které se musí vykreslit stejně (u `mismatch` naopak rozdílně).

## Metoda

- Sparse checkout WPT: pouze `css/css-color`.
- Obě stránky vykreslené přes `jkva-shot` na 800×600.
- Přesné porovnání pixelů, bez tolerance.
- Stránka, která potřebuje externí stylesheet, skript, obrázek, iframe, média, `@import` nebo `url()`, se hlásí jako **unsupported**, nikdy jako pass: jinak by obě strany přišly o stejný zdroj a porovnaly se jako stejné ze špatného důvodu.

```bash
git clone --depth 1 --filter=blob:none --sparse https://github.com/web-platform-tests/wpt /tmp/wpt
cd /tmp/wpt && git sparse-checkout set css/css-color
cd - && JKVA_SHOT=target/debug/jkva-shot \
  cargo run -p wpt-reftest-runner -- /tmp/wpt/css/css-color --limit 25
```

## 2026-10-25 — 25 reftestů

| Výsledek | Počet |
|---|---|
| pass | 11 |
| fail | 6 |
| unsupported | 7 |
| error | 1 |

## Co toto číslo není

Není to WPT skóre ani tvrzení o conformance.

- Pass může být **sdílené selhání**: pokud test i reference používají funkci, kterou engine neumí, vykreslí se stejně a dvojice „projde". Harness odstraní nejzřetelnější příčinu (externí závislosti), ne všechny.
- Jeden error vznikl kvůli referenci mimo sparse checkout (`/css/reference/...`), ne vykreslením.
- Oficiální harness `wpt run` s produktovým adaptérem a WebDriverem přijde s JS enginem v M4.

Poctivé čtení: binárka funguje, corpus runner funguje a číslo je spodní odhad se známým zkreslením nahoru. Nahradí ho oficiální harness, ne další leštění. Druhý běh po doplnění hsl/rgb/named barev posunul 10 na 11 pass; zbylé chyby stojí na barevných prostorech `color()`, které engine neumí.
