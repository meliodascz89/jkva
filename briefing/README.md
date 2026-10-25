# Briefing pro v0.1

Tato složka je archiv rozhodovacího procesu před zahájením vývoje jádra **JKva**. Není to návod k použití ani živý dokument; výsledná pravidla jsou v [`DECISIONS.md`](DECISIONS.md).

## Co je co

| Soubor | K čemu |
|---|---|
| [`BRIEFING-0.1.cs.md`](BRIEFING-0.1.cs.md) | Čitelná verze pro GitHub: kontext, čísla, rizika a všech 99 otázek s označenými volbami. |
| [`ANSWERS.md`](ANSWERS.md) | Vyplněné odpovědi (99/99) a seznam odchylek od doporučení. |
| [`DECISIONS.md`](DECISIONS.md) | Závazná rozhodnutí (ADR), z nichž se odvíjí repo a milníky. |
| [`q/*.json`](q/) | Zdroj pravdy: otázky, možnosti, doporučení, poznámky. |
| [`00-preamble.cs.md`](00-preamble.cs.md) | Úvodní text briefingu. |
| [`questionnaire.html`](questionnaire.html) | Interaktivní dotazník (archiv; odpovědi jsou uzavřené). |
| [`ANSWERS-template.md`](ANSWERS-template.md) | Prázdný odpovědní arch pro případnou revizi. |
| [`build.py`](build.py) | Generátor: `python3 briefing/build.py` vytvoří Markdown, HTML a šablonu odpovědí. |

## Poznámky k údržbě

- Otázky se needitují v Markdownu ani v HTML — jen v `q/*.json`, pak `python3 build.py`.
- `build.py` validuje, že každá single-choice otázka má právě jedno doporučení, a že nejsou duplicitní ID.
- Pořadí sekcí řídí pole `order`, pořadí otázek pole v `questions`.
