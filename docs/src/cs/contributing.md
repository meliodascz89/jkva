# Přispívání

Krátká verze je v [`CONTRIBUTING.md`](https://github.com/jkva/jkva/blob/main/CONTRIBUTING.md). Tahle stránka přidává pravidla specifická pro engine.

## Změna rozhodnutí

Architektonická rozhodnutí jsou v [`briefing/DECISIONS.md`](https://github.com/jkva/jkva/blob/main/briefing/DECISIONS.md). Pokud změna odporuje některému z nich, napiš nejdřív nové ADR do `decisions/` a prober ho v pull requestu. Kód následuje ADR, nikdy naopak.

## Testy, které mají váhu

- Oprava parseru potřebuje nejmenší HTML/CSS vstup, který chybu reprodukuje.
- Oprava layoutu potřebuje referenční screenshot nebo číselnou asertaci, ne „vypadá to dobře“.
- Změna výkonu potřebuje číslo před/po z `benches/`, na stroji uvedeném v reportu.
- Změna paměti potřebuje číslo z `footprint`, pokud se dotýká cache nebo aren.

## Dokumentace

Veřejné položky se dokumentují anglicky. Uživatelské průvodce existují anglicky a česky; když měníš chování, aktualizuj obojí, jinak česká stránka tiše zestárne. Česká verze může být kratší, ale nesmí odporovat anglické.

Tahle kniha se sestavuje přes `mdbook build docs` a stejný příkaz pouští i CI job `docs`. mdBook ale nekontroluje, jestli odkaz vede na stránku, která existuje. Překlep se sestaví bez chyby a v prohlížeči skončí na 404, tak si odkazy před pushem projdi sám.
