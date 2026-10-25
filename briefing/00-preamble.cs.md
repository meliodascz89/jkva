# JKva 0.1 — briefing pro vývoj jádra prohlížeče

**Stav:** rozhodnuto 2026-10-25 · **Autor:** Lead (DeepSeek Harness) · **Rozsah:** 99 otázek v 18 sekcích · **Závazný výstup:** [DECISIONS.md](DECISIONS.md)

Tento dokument je poslední krok před prvním commitem. Rozhoduje o tom, co vlastně stavíme, co k tomu použijeme, jak to změříme a co z toho **neděláme**. Odpovědi se stanou ADR záznamy (`DECISIONS.md`), ze kterých se pak odvíjí repo, milníky a rozdělení práce agentům.

**Co tento projekt je a co není.** Vyvíjíme pouze **jádro JKva** — knihovnu, která umí načíst, spočítat a vykreslit web. Prohlížeč (okna, taby, správa procesů, historie, stahování, aktualizace, UI) je **samostatný projekt**, který vznikne později a bude jádro konzumovat přes veřejné API. Referenční shell v tomto projektu je jen testovací a demo harness, ne začátek budoucího prohlížeče.

---

## Jak odpovídat

> **Vyplněno 2026-10-25.** Tato sekce zůstává jako návod pro budoucí revizi rozhodnutí; zvolené varianty jsou vyznačené u každé otázky.

1. **Interaktivní dotazník** — otevři `briefing/questionnaire.html` v prohlížeči. Klikáš, doplňuješ poznámky, tlačítkem **Export odpovědí** dostaneš text, který mi pošleš. Odpovědi se ukládají lokálně, takže se nic neztratí.
2. **Markdown** — tento soubor. Odpověz ve formátu `Q01 = A`, u vícenásobných `Q40 = A,B,C,D`. Příloha B obsahuje prázdný odpovědní arch.
3. **Rozhovor** — řekni „projdi to se mnou" a projdeme otázky po sekcích; já se ptám a ty jen potvrzuješ nebo měníš.

**Klíčové pravidlo:** u každé otázky je ✅ označená **doporučená** varianta. Pokud něco necháš prázdné, použiju doporučené – ale v `DECISIONS.md` to bude výslovně označeno jako „přijato implicitně", ne jako tvoje volba. U otázek, které mění rozsah (Q07, Q08, Q42, Q71), implicitní přijetí nedělám a vrátím se k nim.

**Kolik to zabere:** 15–25 minut, pokud jen potvrdíš doporučené. 45–60 minut, pokud chceš projít vše poctivě. Příloha A uvádí 13 otázek, které rozhodují nejvíc.

---

## TL;DR — deset rozhodnutí, na kterých to stojí

1. **Rozsah 0.1 = browser-grade engine, 36–48 měsíců, 400–450 tis. řádků** (Q07, Q07b, Q08). Celý engine včetně tabulek podle specifikace, Shadow DOM, OOPIF, multi-procesu, videa, WebGL a platformních API.
2. **Nepíšeme HTML/CSS parsery, shaping ani JS engine.** Píšeme DOM, kaskádu, layout, paint, kompozitor, event loop a bindingy (Q36, Q39, Q51, Q62).
3. **JS engine = SpiderMonkey (`mozjs`)**, ne V8 (Q62). Důvod není jen výkon: je to jediná knihovna bindingů s přesným GC trasováním Rust objektů a jediná cesta k poctivému srovnání „náš engine vs. Firefox" se stejným JS. Je to jednosměrné rozhodnutí.
4. **Inkrementální layout a invalidační seam od prvního měsíce** (Q48). Dodatečně se to nezavádí – Servo to ladí 14 let.
5. **Areny pro DOM a layout, `footprint` jako měřítko paměti** (Q21, Q28, Q73). Slib „nižší paměť" musí být měřený, ne pocitový.
6. **Speedometer je od 0.1 blokující brána s cílem ≥ 25** (Q71, Q72). Je to možné jen díky tomu, že shadow DOM, contenteditable, iframy, canvas i SVG jsou v rozsahu.
7. **Licence MIT OR Apache-2.0**, MPL-2.0 závislosti (cssparser, selectors, stylo, mozjs) zůstávají závislostmi (Q12).
8. **Veřejné od prvního commitu**, Conventional Commits + generovaný changelog; komunikace při milnících (Q13, Q14, Q97), kompenzovaná častými releases a veřejným dashboardem.
9. **Dokumentace EN kanonická + CS pro průvodce**, `deny(missing_docs)` a SAFETY komentáře vynucené kompilátorem (Q84, Q85).
10. **4 stálí agenti + Lead**, každý merge přes review a brány; memory drží rozhodnutí, graphify hlídá architektonický drift (Q90, Q91, Q93).

---

## Realita v číslech

Tohle není odrazování, je to kalibrace. Čísla jsou ověřená z primárních zdrojů (viz `research/03-prior-art.md`).

| Projekt | Jazyk | Stáří | Řádky jádra | Stav | Lekce |
|---|---|---|---|---|---|
| **WebKit** | C++ | 20+ let | ~1,9 mil. | produkce (Safari) | referenční měřítko rozsahu |
| **Gecko** | C++/Rust | 25+ let | 1,5+ mil. | produkce (Firefox) | Rust pronikl do stylů a renderu, ne všude |
| **Ladybird** | C++/Rust | 7,3 roku | ~1,19 mil. | alpha stále nevydaná; Speedometer 3 ≈ 5,9 | i funded tým s 1,2 mil. řádky je po 7 letech daleko od prohlížeče |
| **Servo** | Rust | 14,7 roku | ~544 tis. | nightly, „neloguj se do banky" | crates jsou skutečný přínos; shell a stabilita jsou jiná liga |
| **Blitz** | Rust | ~2,5 roku seriózně | ~68–80 tis. | beta, **bez JS** | nejbližší analog naší architektury; reuse nezaručuje dokončení |
| **Aurora** | Rust | 7 měsíců | ~28,6 tis. | prototyp | za půl roku je integrační vrstva, ne engine |

**Co z toho plyne:** „from scratch" v našem pojetí neznamená znovuobjevit HTML5 parsing. Znamená vlastnit to, co je skutečné know-how jádra – a to je podle všeho **pět věcí**: DOM graf s GC, box/inline layout a řádkování, invalidační seam, display list s kompozitorem, a event loop s životním cyklem dokumentu. Všechno ostatní je buď komodita (parsery, shaping, HTTP), nebo věc, kterou lze koupit za závislost.

### Co reusujeme a co píšeme

| Vrstva | Kdo to napíše | Poznámka |
|---|---|---|
| HTML parser | `html5ever` | MIT/Apache, spec-compliant, 133 mil. stažení |
| CSS tokeny/selektory | `cssparser`, `selectors` | MPL-2.0; kaskádu si ale píšeme sami (nebo Stylo – Q39) |
| Kaskáda a computed values | **my** (případně `stylo`) | největší jednotlivé rozhodnutí; Stylo = roky správnosti, ale MPL a týdny lepení |
| Flexbox / grid | `taffy` | MIT; neumí inline, text ani tabulky |
| Blok/inline/float/table layout | **my** | skutečná IP; nejtěžší část je inline a řádkování |
| Shaping | `harfrust` | nástupce archivovaného rustybuzzu, parita s HarfBuzz 15 |
| Fonty a rasterizace | `skrifa`, `fontique`, `glifo`/`vello_gpu` | Firefox používá skrifa; glyfový atlas je náš |
| GPU a rastr | `wgpu` 30 + `vello_gpu`/`vello_cpu` | Metal je default; wgpu breaking verze každé čtvrtletí |
| JS engine | `mozjs` (SpiderMonkey 153) | MPL-2.0, TraceTrait, prebuilt archivy |
| WebIDL bindingy | **my** (generátor) | bez generátoru se stovky rozhraní nedají udržet |
| Síť | `hyper`, `rustls`, `h2`, `quinn` | Firefox používá hyper; QUIC až 0.2 |
| Cache a cookies | `http-cache-semantics` + **náš obal** | `cookie_store` je od února bez commitů |
| SVG | `resvg` | render, ne styling |
| AI export | **my** | sémantický strom + akční API, žádné modely v jádře |

---

## Navržená architektura (workspace)

Název `jkva` je daný, viz Q06.

```text
jkva/
├── Cargo.toml                  # workspace, edition 2024, arm64-first
├── crates/
│   ├── jkva-core/               # ID, areny, fronty, scheduler, diagnostika, signposty
│   ├── jkva-platform/           # macOS: QoS vlákna, soubory, schránka, Metal device
│   ├── jkva-net/                # hyper + rustls + cache + cookies + proxy
│   ├── jkva-dom/                # html5ever TreeSink → arena DOM, eventy, shadow slot
│   ├── jkva-style/              # CSS kaskáda, computed values, media queries, invalidace
│   ├── jkva-layout/             # box/fragment tree; blok, inline, float, table + taffy
│   ├── jkva-text/               # shaping, fallback, řádkování, glyfový atlas
│   ├── jkva-paint/              # display list, stacking contexts, damage
│   ├── jkva-gfx/                # wgpu, vello_gpu, vrstvy, kompozitor, present
│   ├── jkva-js/                 # mozjs, WebIDL codegen, bindingy, GC tracing
│   ├── jkva-api/                # fetch, storage, observers, timery, history
│   ├── jkva-engine/             # dokument, lifecycle, event loop, navigace, embed API
│   └── jkva-shell/              # referenční okno: URL bar, zpět/vpřed, dev overlay
├── xtask/                      # build, release, benchmark, WPT orchestrace
├── tools/                      # wpt-runner, reftest-diff, bench-tracker (Q92)
├── benches/                    # criterion + korpusové benchmarky
├── docs/                       # mdBook EN + CS, ADR, architektura
└── research/                   # tento briefing a podklady
```

Pravidlo: `jkva-core` nesmí záviset na macOS. `jkva-platform` je jediné místo s `objc2`/Metal. Díky tomu běží CI na Linuxu a jádro jde později portovat.

---

## Jak budeme měřit úspěch

Metodika a čísla jsou v `research/02-benchmarks.md`. Toto je pracovní matice, kterou budeme publikovat:

| # | Metrika | Postup | Cíl 0.1 | Konkurence (M2/M4) |
|---|---|---|---|---|
| 1 | Studený start → interaktivní okno | `os_signpost`, čistý profil, n=30 | **p50 ≤ 400 ms** | Safari <1 s, Chrome 1–2 s |
| 2 | First Contentful Paint, lokální korpus | lokální HTTP, cold cache, n=20×3 | **≤ 1,3× Chrome** | Chrome |
| 3 | Speedometer 3.1 | `crossbench`, medián z 5 | **≥ 25** (brána) | 41,9 / 52 / 61 |
| 4 | MotionMark 1.3.2 | 60 Hz displej, subtests | **≥ 1 500** – publikujeme jako deficit | 8 089 (Safari) |
| 5 | WPT aligned | `wpt run` + wptreport | **55 %** (DOM 80 / HTML 75 / CSS 40) | 97,1 / 94,5 / 93,5 |
| 6 | Paměť při 20 dokumentech | `footprint -t -j`, fyzický footprint | **≤ 1,15× Chrome**, ≤ 90 MB/kartu | měřit, nehádat |
| 7 | Energie při videu | externí power meter, jinak battery A/B | **parita ±10 %** | Safari |
| 8 | Velikost binárky | `du -sm`, stripped | **≤ 120 MB** | Chrome 1,44 GB / Safari stub 35 MB |

**Poctivostní pravidla:** mediány, ne průměry; bootstrap CI; stejný stroj, stejný den, interleaved A/B/C; zamčené verze (macOS build, SHA enginu, verze JS engine, WPT revize); vždy uvést, co jsme obětovali (single-process = slabší izolace). Tvrzení se **předregistrují** před měřením, ne vybírají po něm.

---

## Osm rizik, která rozhodují o úspěchu

| # | Riziko | Proč je vážné | Mitigace |
|---|---|---|---|
| 1 | **Inline a textový layout** | Taffy ho neumí; Blitz na něm strávil tři release cykly; Servo v roce 2026 opravoval shaping přes inline boxy | `parley`/ICU na segmentaci, vlastní box model, od M1 testy proti referenčnímu renderu |
| 2 | **GC a cykly DOM↔JS** | Ladybird unikal 15 GB/20 h; Servo dělá rozsáhlý rewrite JS integrace | `mozjs` TraceTrait, návrh kolektorů **před** bindingy, leak testy v CI |
| 3 | **Inkrementální layout nelze dohnat** | Servo 14 let ladí dirty roots; bez toho není Speedometer ani scroll | invalidační seam v M1, i když první verze počítá víc, než musí |
| 4 | **Tabulky, floaty a margin collapsing** | Provázané se vším; Blitz tabulky emuluje gridem | timebox, základní verze, zbytek vědomě mimo 0.1 |
| 5 | **Shadow DOM pro Speedometer** | Bez něj benchmark neprojde; s ním +3–4 měsíce | Q71 rozhoduje, zda Speedometer odložit – doporučuji odložit |
| 6 | **wgpu breaking každé čtvrtletí** | ~týden údržby na major verzi | tenká abstrakce, pin, upgrade v rámci M4 |
| 7 | **Stylo integrace a MPL** | API se hýbe (0.19→0.22 za 3 měsíce), týdny lepení | týdenní spike v M0, rozhodnutí podle dat (Q39) |
| 8 | **Rozsah se tiše nafoukne** | „Když už umíme X, proč ne Y" | předem odsouhlasené pořadí řezů (Q10) a definice hotového (Q94) |

---

## Jak bude projekt řízen

- **Já jako Lead:** architektura, rozhodnutí, review každého mergu, integrace, release, dokumentace. Nedělám „další funkci" – držím trajektorii.
- **Čtyři stálí agenti** (Q90): core/arch, layout+text, paint+GPU, JS bindingy+API. Každý vlastní svůj crate a write scopes, aby si nešlapali po souborech.
- **Brány:** CI zelené, testy, clippy `-D warnings`, `deny(missing_docs)`, review Leadem, žádný direct push do `main` (Q91).
- **Nástroje:** WPT runner, reftest diff a bench tracker jako interní pluginy harnessu (Q92) – ušetří víc času než další CSS vlastnost.
- **Paměť a graf:** každé rozhodnutí z briefingu → memory + ADR; na začátku milníku graphify kontrola architektonického driftu (Q93).
- **Rytmus:** komunikace při milnících (Q97), ale každý milník má release, benchmark report a devlog – jinak projekt působí mrtvě.

---

## Podklady

- [`research/01-stack-audit.md`](research/01-stack-audit.md) — 59 cratů: verze, licence, údržba, produkční použití (Servo/Firefox/Blitz/Ladybird). Obsahuje čtyři korekce našich předpokladů (rustybuzz archivován, html5ever není MPL, Stylo je publikované, Vello se restrukturalizovalo).
- [`research/02-benchmarks.md`](research/02-benchmarks.md) — benchmarky 2026 (Speedometer 3.1, JetStream 3.0, MotionMark 1.3.2), WPT mechanika a čísla, paměť/energie na macOS, metodika a matice.
- [`research/03-prior-art.md`](research/03-prior-art.md) — Servo, Blitz, Aurora, Ladybird: LOC, roky, co pokrývají, deset chyb, kterých se dopustily.

---

## Legenda a poznámky

- **`Qnn`** — identifikátor otázky, používej ho v odpovědích.
- **✅ doporučeno** — varianta, kterou umím obhájit a kterou bych zvolil.
- **„jiné"** — u některých otázek můžeš napsat vlastní variantu; pole je v dotazníku i v odpovědním archu.
- **Poznámka** — cokoli doplňujícího; u klíčových otázek to beru jako součást odpovědi.
- Odkazy typu „viz Q48" míří na jinou otázku v tomto dokumentu.
