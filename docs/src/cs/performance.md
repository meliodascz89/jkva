# Výkonnostní baseline

Každé číslo zde uvádí fixture, počet iterací, stroj a příkaz. Číslo bez toho není měření.

## 2026-10-25 — baseline M1

- Stroj: Apple M4, macOS 27.0.1, rustc 1.99.1 (stable)
- Fixture: 200 řádků (602 uzlů DOM), 200 CSS pravidel, 16 221 B HTML, 9 413 B CSS
- Iterace: 20, uveden medián
- Příkaz: `cargo run -p bench-runner`

| Bench | Medián |
|---|---|
| parse-html | 3,02 ms |
| parse-css | 0,82 ms |
| cascade | 19,93 ms |
| shape-text | 33,8 µs |

## Co ta čísla znamenají

Jsou to mikro-benchmarky vrstev, ne Speedometer a ne srovnání s Chromem. Nejhorší je `cascade`: styly se matchují průchodem všech pravidel pro každý element, bez indexu pravidel a bez sdílení stylů. To je známý úkol pro M2 a tahle tabulka má dokázat zlepšení, až přijde.

## Pravidla

- Při změně parseru, stylů nebo textu spusť stejnou fixture.
- Nikdy necituj číslo bez velikosti fixture a stroje.
- Regresi publikuj stejně hlasitě jako zlepšení.
