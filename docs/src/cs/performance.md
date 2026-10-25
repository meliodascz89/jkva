# Výkonnostní baseline

Každé číslo zde uvádí fixture, počet iterací, stroj a příkaz. Číslo bez toho není měření.

## 2026-10-25 — baseline M1

- Stroj: Apple M4, macOS 27.0.1, rustc 1.99.1 (stable)
- Fixture: 200 řádků (602 uzlů DOM), 200 CSS pravidel, 16 221 B HTML, 9 413 B CSS
- Iterace: 20, uveden medián
- Příkaz: `cargo run -p bench-runner`

| Bench | Medián |
|---|---|
| parse-html | 2,46 ms |
| parse-css | 0,80 ms |
| cascade | 2,60 ms |
| shape-text | 30,5 µs |

## Co ta čísla znamenají

Jsou to mikro-benchmarky vrstev, ne Speedometer a ne srovnání s Chromem.

Kaskáda bývala nejhorší číslo: **19,93 ms**, protože se každé pravidlo testovalo proti každému elementu. Index pravidel podle id, class a typu (nejpravější složený selektor, s fallbackem na universal) ji dostal na 2,60 ms na stejné fixture — 8× v debugu a 2,2× v release. Co zbývá, je alokace na element, ne matching; to je další měřený krok.

## Pravidla

- Při změně parseru, stylů nebo textu spusť stejnou fixture.
- Nikdy necituj číslo bez velikosti fixture a stroje.
- Regresi publikuj stejně hlasitě jako zlepšení.
