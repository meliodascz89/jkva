# Briefing pro v0.1 — jak to použít

Tato složka obsahuje rozhodovací dokument před zahájením vývoje jádra **JKva**.

## Co je co

| Soubor | K čemu |
|---|---|
| [`questionnaire.html`](questionnaire.html) | **Interaktivní dotazník** – otevři v prohlížeči, klikni odpovědi, exportuj text. Doporučená cesta. |
| [`BRIEFING-0.1.cs.md`](BRIEFING-0.1.cs.md) | Čitelná verze pro GitHub: kontext, čísla, rizika + všech 99 otázek s označenými volbami. |
| [`ANSWERS.md`](ANSWERS.md) | **Vyplněné odpovědi** (99/99) a seznam odchylek od doporučení. |
| [`ANSWERS-template.md`](ANSWERS-template.md) | Prázdný odpovědní arch pro budoucí revizi. |
| [`DECISIONS.md`](DECISIONS.md) | **Závazná rozhodnutí** (ADR) – z nich se odvíjí repo a milníky. |
| [`FUNDING.md`](FUNDING.md) | Text podpory projektu (EN+CS) pro repo; adresa se doplní při zveřejnění. |
| [`GITHUB-PROFILE.md`](GITHUB-PROFILE.md) | Návrh popisu repa, taglinů a úvodu README (EN+CS). |
| [`q/*.json`](q/) | **Zdroj pravdy** – otázky, možnosti, doporučení, poznámky. |
| [`00-preamble.cs.md`](00-preamble.cs.md) | Úvodní text briefingu. |
| [`build.py`](build.py) | Generátor: `python3 briefing/build.py` vytvoří MD, HTML i šablonu odpovědí. |

## Jak odpovídat

1. Otevři `questionnaire.html` (dvojklik nebo `open briefing/questionnaire.html`).
2. Projdi sekce, u každé otázky klikni variantu; doporučená je zeleně označená.
3. Tlačítkem **Export odpovědí** zkopíruj text (nebo ulož `ANSWERS.md`) a pošli mi ho.
4. Odpovědi se ukládají do `localStorage`, takže zavření prohlížeče nic neztratí.

Alternativa bez prohlížeče: odpovídej ve formátu `Q01 = A`, multi `Q40 = A,B,C,D`.

## Co se stane po vyplnění

1. Vygeneruji `DECISIONS.md` — jedno ADR na rozhodnutí, s odůvodněním a dopady.
2. Založím veřejný repozitář: workspace, CI, dokumentace EN+CS, šablony, roadmapa.
3. Postavím M0 walking skeleton (první pixely přes vello, CI, harness).
4. Rozdělím práci čtyřem agentům a nastavím brány.

## Poznámky k údržbě

- Otázky se needitují v Markdownu ani v HTML — jen v `q/*.json`, pak `python3 build.py`.
- `build.py` validuje, že každá single-choice otázka má právě jedno doporučení, a že nejsou duplicitní ID.
- Pořadí sekcí řídí pole `order`, pořadí otázek pole v `questions`.
