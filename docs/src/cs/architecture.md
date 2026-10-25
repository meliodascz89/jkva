# Architektura

JKva je Cargo workspace s jednou cratí na vrstvu. Závislosti ukazují jen dolů; `jkva-core` neví nic o platformě a `jkva-platform` je jediná crate, která sahá na macOS frameworky.

| Crate | Vlastní |
|---|---|
| `jkva-core` | identifikátory, areny, plánovací primitiva, diagnostiku |
| `jkva-platform` | Metal zařízení, QoS vláken, signposty, schránku, soubory |
| `jkva-net` | HTTP/1.1, HTTP/2, HTTP/3, TLS, cache, cookies |
| `jkva-dom` | HTML strom, eventy, shadow roots |
| `jkva-style` | CSS parsing, kaskádu, computed values, invalidaci |
| `jkva-layout` | box tree, blok/inline/float/table layout, fragmentaci |
| `jkva-text` | shaping, fallback fontů, řádkování, glyfový atlas |
| `jkva-paint` | display listy, stacking contexts, damage |
| `jkva-gfx` | záznam příkazů, Metal 4 rasterizér, vrstvy, kompozitor |
| `jkva-js` | integraci SpiderMonkey, Web IDL bindingy, GC trasování |
| `jkva-api` | Web API postavené nad enginem |
| `jkva-engine` | životní cyklus dokumentu, event loop, navigaci, embedder API |
| `jkva-shell` | referenční demo shell (ne budoucí prohlížeč) |

## Pravidla

- Žádná crate nad `jkva-platform` nesmí záviset na macOS frameworcích.
- Žádné `unsafe` bez komentáře `// SAFETY:`; Miri běží nad unsafe povrchem.
- Veřejné položky mají dokumentaci; `missing_docs` je chyba.
- Změna hranic je ADR. Viz [`briefing/DECISIONS.md`](https://github.com/jkva/jkva/blob/main/briefing/DECISIONS.md).
