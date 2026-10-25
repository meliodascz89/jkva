[English](../en/index.md) | **Čeština**

# JKva

JKva je browser engine psaný od nuly v Rustu pro Apple Silicon. Není to fork Chromia a neobaluje existující engine.

Projekt je ve fázi **M0 (základy)**. Repo, CI a probe z `jkva-platform` existují; první vykreslená stránka přijde v M1.

## Principy

1. **Engine vlastníme, standardy kupujeme.** Parsery, shaping, JS engine a GPU API jsou závislosti. DOM, kaskádu, layout, paint, kompozitor a event loop si píšeme sami.
2. **Nejdřív měřit, potom tvrdit.** Každé tvrzení o výkonu se publikuje i s metodou a strojem, na kterém vzniklo.
3. **Rozhodovat veřejně.** Architektonická rozhodnutí jsou ADR, ne rozhovory na chodbě.
4. **Paměť je funkce.** Areny, rozpočty a metrika footprintu od prvního dne.

## Začni tady

- [Architektura](architecture.md) — co která crate vlastní.
- [Sestavení](building.md)
- [Výkonnostní baseline](performance.md) — jak zkompilovat a spustit probe.
- [Nástroje](tools.md) — dva nástroje pro příkazovou řádku: referenční testy a výsledky WPT.
- [Přispívání](contributing.md) — jak poslat změnu.
- [`briefing/DECISIONS.md`](https://github.com/meliodascz89/jkva/blob/main/briefing/DECISIONS.md) — 24 ADR, na kterých to celé stojí.
