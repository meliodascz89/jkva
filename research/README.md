# Podklady k briefingu

Tři rešeršní dokumenty, ze kterých vychází rozhodnutí v `briefing/BRIEFING-0.1.cs.md`. Všechna čísla mají v původních dokumentech uvedený zdroj a datum ověření (2026-10-25).

| Dokument | Co obsahuje | Nejdůležitější zjištění |
|---|---|---|
| [`01-stack-audit.md`](01-stack-audit.md) | 59 Rust cratů: verze, datum vydání, licence, údržba, arm64, produkční použití (Servo, Firefox, Ladybird, Blitz) | `rustybuzz` je archivovaný → `harfrust`; `html5ever` je MIT/Apache, MPL je jen `cssparser`/`selectors`/`stylo`/`mozjs`/`lightningcss`; `vello` 0.11 je experimentální, produkční je `vello_gpu`/`vello_cpu`; `taffy` neumí inline/text/tabulky |
| [`02-benchmarks.md`](02-benchmarks.md) | Benchmarky 2026 (Speedometer 3.1, JetStream 3.0, MotionMark 1.3.2), WPT mechanika a naměřené pass rate, paměť a energie na macOS, metodika a 8řádková matice | Speedometer vyžaduje shadow DOM, contenteditable, iframy, canvas a SVG; WPT: Chrome 97,1 / Firefox 94,5 / Safari 93,5 / Servo 88,3 (aligned); Apple EU gate = 90 % WPT + 80 % test262 i s vypnutým JIT; `footprint`, ne RSS |
| [`03-prior-art.md`](03-prior-art.md) | Servo, Blitz, Aurora, Ladybird: stáří, LOC, pokrytí, licence, deset chyb a podcenění | Ladybird 1,19 mil. LOC / 7,3 roku / Speedometer ~5,9; Blitz po 2,5 letech ~68–80 tis. LOC, beta, **bez JS**, tabulky emuluje gridem; Servo 544 tis. LOC / 14,7 roku |

## Jak číst čísla

- **LOC** = řádky kódu jádra, ne celého repozitáře (testy, dokumentace a nástroje nejsou zahrnuté).
- **Speedometer** = verze 3.1; čísla z různých strojů (M2/M4/M5) nejsou vzájemně srovnatelná.
- **WPT aligned** = podíl prošlých subtestů vůči nejvyššímu počtu, který daný test spustil některý engine; vlastní jmenovatelé se liší.
- **Paměť** = fyzický footprint (`footprint`), nikdy VSZ ani samotné RSS.

## Co z toho plyne pro plán

1. Parsery, shaping, HTTP a JS engine jsou komodity — kupujeme.
2. DOM s GC, inline layout, invalidační seam, display list a event loop jsou naše IP.
3. Bez inkrementálního layoutu a hygieny event loopu není výkon; bez `mozjs` TraceTrait není bezpečná GC.
4. „Překonat Blink ve všem\" není cíl na 12 měsíců. Reálné je vyhrát start, paměť na kartu, velikost binárky a poctivě měřený delta proti Firefoxu se stejným JS.
