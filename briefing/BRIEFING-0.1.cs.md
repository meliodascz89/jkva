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

---

## Obsah

1. [Produkt, korpus a měřítko úspěchu](#1-produkt-korpus-a-meritko-uspechu) — 6 otázek
2. [Realita, ambice a časový horizont](#2-realita-ambice-a-casovy-horizont) — 6 otázek
3. [Licence, veřejnost a podoba repozitáře](#3-licence-verejnost-a-podoba-repozitare) — 4 otázek
4. [Architektura a hranice systému](#4-architektura-a-hranice-systemu) — 6 otázek
5. [Apple Silicon, GPU a vlákna](#5-apple-silicon-gpu-a-vlakna) — 6 otázek
6. [Paměťová strategie](#6-pametova-strategie) — 4 otázek
7. [Síťová vrstva](#7-sitova-vrstva) — 5 otázek
8. [HTML, DOM a CSS](#8-html-dom-a-css) — 9 otázek
9. [Layout engine](#9-layout-engine) — 6 otázek
10. [Text, fonty a typografie](#10-text-fonty-a-typografie) — 6 otázek
11. [Vykreslování, kompozice a vizuální kvalita](#11-vykreslovani-kompozice-a-vizualni-kvalita) — 5 otázek
12. [JavaScript, WebAssembly a bindingy](#12-javascript-webassembly-a-bindingy) — 5 otázek
13. [Web API surface a formuláře](#13-web-api-surface-a-formulare) — 4 otázek
14. [Měření: jak prokážeme, že jsme dobří](#14-mereni-jak-prokazeme-ze-jsme-dobri) — 6 otázek
15. [Bezpečnost, sandbox a soukromí](#15-bezpecnost-sandbox-a-soukromi) — 4 otázek
16. [AI a agentní vrstva](#16-ai-a-agentni-vrstva) — 3 otázek
17. [Dokumentace, CI a release](#17-dokumentace-ci-a-release) — 6 otázek
18. [Proces: agenti, pluginy a definice hotového](#18-proces-agenti-pluginy-a-definice-hotoveho) — 8 otázek

Celkem **99 otázek** v 18 sekcích.

---

<a id="1-produkt-korpus-a-meritko-uspechu"></a>

## 1. Produkt, korpus a měřítko úspěchu

Než vznikne první řádek kódu, musíme zamknout, co je vlastně produkt, na jakém korpusu webů se bude poměřovat a co znamená „překonat nejlepší". Bez toho se projekt rozpadne na nekonečný výzkum, protože každý týden půjde definovat „ještě jeden standard".

Tato sekce rozhoduje o 30–50 % celkového rozsahu práce. Odpověz prosím i tehdy, když si nejsi jistý – u každé otázky je označená doporučená varianta, kterou umím obhájit.

### Q01 — Co je výstupem verze 0.1?

*vyber jednu*

**Proč to řešíme:** Jádro bez shellu se nedá předvést, změřit na reálných webech ani ukázat veřejnosti. Plnohodnotný prohlížeč je ale samostatný projekt, který v tomto projektu neřešíme – vyvíjíme jen JKva a tenký demo shell.

- [x] **A** — Knihovna (Rust crates) + referenční minimální shell — ✅ **doporučeno** · **zvoleno**
  - Shell = jedno okno, adresní řádek, zpět/vpřed, reload, dev overlay. Slouží k demu, benchmarkům a screencastům; jádro zůstává čistě knihovna s veřejným API. Není to začátek budoucího prohlížeče.
- [ ] **B** — Pouze knihovna/crates, bez shellu
  - Nejmenší rozsah, ale nemáme co ukázat veřejnosti ani jak měřit reálné weby a plynulost scrollu.
- [ ] **C** — Plnohodnotný prohlížeč (jádro + browser UI)
  - Taby, historie, stahování, nastavení, rozšíření, aktualizace. Minimálně 2–3× větší rozsah a úplně jiný typ práce (UI, produkt, podpora).
- [ ] **D** — Headless engine pro automatizaci a AI agenty
  - Bez UI, API pro řízení a extrakci obsahu. Zajímavé a komerčně cenné, ale vizuální kvalitu ani plynulost na tom neprokážeme.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q02 — Jaký korpus webů musí v0.1 vykreslit správně?

*vyber vše, co platí*

**Proč to řešíme:** Korpus je definice „hotovo". Čím širší, tím víc okrajových případů – a tím později je verze hotová. Doporučuji kombinaci, která pokrývá reálné weby i benchmarky.

- [x] **A** — Top-100 webů podle Tranco — ✅ **doporučeno** · **zvoleno**
  - Statické vykreslení + porovnání screenshotů se Safari/Chromem. Uzavřený, měřitelný cíl a silná demonstrace.
- [x] **B** — Top-1000 webů · **zvoleno**
  - Odhalí výrazně víc okrajových případů, ale každá další stovka je drahá. Vhodné jako cíl pro 0.3+, ne pro 0.1.
- [x] **C** — 20 interaktivních aplikací (React/Vue/Svelte/Angular) — ✅ **doporučeno** · **zvoleno**
  - Todo, dashboard, tabulka s filtrováním. Přesně to, co měří Speedometer – bez toho benchmark neodpovídá realitě.
- [x] **D** — Dokumentační a long-form weby (MDN, RFC, Wikipedie) — ✅ **doporučeno** · **zvoleno**
  - Text, kotvy, tabulky, bloky kódu, dlouhé stránky, scroll. Nejlepší test typografie a layoutu.
- [x] **E** — E-shopy a formulářové vzory · **zvoleno**
  - Formuláře, validace, tabulky, obrázky, košík. Náročné na formulářové prvky a přístupnost.
- [x] **F** — Zpravodajské servery · **zvoleno**
  - Obrázky, lazy-loading, reklamní skripty, časté reflow. Bez reklam je to hlavně test obrázků a textu.
- [x] **G** — Vlastní „dream corpus" definovaný později · **zvoleno**
  - Riziko posouvání cíle: korpus, který se mění podle toho, co zrovna funguje, není definice hotového.
- [x] **H** — Umělé micro-benchmarky (Speedometer workloads) — ✅ **doporučeno** · **zvoleno**
  - Nutné pro měření regresí v CI, ale samotné nedokazují, že jádro umí reálný web.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F, G, H · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q03 — Která veřejná tvrzení o výkonu budeme smět říkat?

*vyber vše, co platí*

**Proč to řešíme:** Tvrzení musí být obhajitelné měřením, jinak projekt ztratí důvěryhodnost u vývojářů. Tato otázka určuje, co půjde do README a na web – a tedy co musíme skutečně změřit.

- [x] **A** — Nižší paměť na kartu než Chrome/Safari — ✅ **doporučeno** · **zvoleno**
  - Naše nejsilnější a nejlépe měřitelná výhoda díky Rustu, arenové alokaci a UMA na Apple Silicon.
- [x] **B** — Nižší spotřeba energie při scrollu a videu — ✅ **doporučeno** · **zvoleno**
  - Měřitelné přes powermetrics; přesně to, na co si uživatelé stěžují u Firefoxu na Macu.
- [x] **C** — Rychlejší studený start a first paint — ✅ **doporučeno** · **zvoleno**
  - Měřitelné a snadno pochopitelné; nové jádro bez 15 let historie má reálnou šanci.
- [x] **D** — Vyšší vizuální věrnost (barvy, AA, 120 Hz) — ✅ **doporučeno** · **zvoleno**
  - Subjektivní část je potřeba podložit metrikami (deltaE, SSIM proti referenčnímu renderu, frame pacing).
- [x] **E** — Skóre v benchmarku – cíl parita, ne vítězství — ✅ **doporučeno** · **zvoleno**
  - Speedometer měří i JavaScript. S SpiderMonkey máme stejný základ jako Firefox, takže rozdíl bude náš layout, styl a paint. Reálně: Safari 26.2 na M2 dává 41,9, Chrome 42,7, Firefox 35,7; Ladybird po 7 letech ~5,9. Cíl pro 0.1 je „projde a změří se", ne vítězství.
- [ ] **F** — Žádná veřejná tvrzení v 0.1, jen interní měření
  - Bezpečné, ale veřejný projekt potřebuje něco, čím zaujme. Tvrzení jde zveřejnit později.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q04 — Jaká je vlajková věta projektu (jedna, do README)?

*vyber jednu*

**Proč to řešíme:** Jedna věta určuje priority při každém sporu o rozsah. Pokud je vlajkou paměť, nebudeme plýtvat na funkce, které žerou RAM. Pokud je to vizuální kvalita, nebudeme optimalizovat každou alokaci.

- [ ] **A** — Nejnižší paměť na kartu mezi moderními jádry
  - Nejlépe obhajitelné, snadno měřitelné, silný příběh pro Apple Silicon.
- [ ] **B** — Nejplynulejší scroll a animace na 120 Hz
  - Krásné demo, ale těžko se dokazuje proti Safari, které je na Macu extrémně dobře optimalizované.
- [ ] **C** — Nejrychlejší studený start a první vykreslení
  - Měřitelné a užitečné, ale méně „sexy" než paměť nebo kvalita.
- [ ] **D** — Nejvěrnější barvy a text na Apple Silicon
  - Dobře ladí s požadavkem „krásné vykreslování"; konkurence tu má rezervy (P3, HDR, font smoothing).
- [x] **E** — Špičková vizuální kvalita při poloviční paměti — ✅ **doporučeno** · **zvoleno**
  - Kombinace dvou výše uvedených – nejzajímavější příběh, ale musíme umět obojí změřit.
- [ ] **F** — Žádná vlajka – jsme výzkumná laboratoř
  - Otevřeně přiznaný výzkum; méně tlaku na výsledky, menší šance zaujmout přispěvatele.
- [ ] **jiné** — ______________________

**Zvoleno:** E · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q05 — Potvrzuješ, že těchto 12 věcí je vědomě mimo 0.1?

*vyber jednu*

**Proč to řešíme:** Explicitní seznam „tohle neděláme" je stejně důležitý jako seznam funkcí. Brání tomu, aby se rozsah tiše nafoukl. Pokud některou chceš už v 0.1, označ B a napiš kterou.

- [x] **A** — Ano, všech 12 je mimo 0.1 — ✅ **doporučeno** · **zvoleno**
  - Rozšíření (adblock, password managery) · synchronizace a účty · správce hesel · tisk a PDF export · DRM (Widevine/FairPlay) · DevTools UI · AI inference uvnitř jádra · multi-procesní izolace · mobilní shell (iOS) · plná accessibility · lokalizace UI · automatické aktualizace.
- [ ] **B** — Ne, některé chci už v 0.1
  - Napiš které a proč – přepočítáme milníky a rozpočet.

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q06 — Potvrzuješ název JKva a rozdělení projektů?

*vyber jednu*

**Proč to řešíme:** Název jde do názvů cratů na crates.io, do GitHubu, domény a binárek. Zároveň platí, že tento projekt dodává jen jádro; prohlížeč je samostatný pozdější projekt, který jádro konzumuje přes API.

- [x] **A** — JKva = jádro; prohlížeč jako samostatný projekt později — ✅ **doporučeno** · **zvoleno**
  - Crates `jkva-*`, binárka `jkva`, repo `jkva`. Shell v tomto projektu je jen demo harness. Doporučeno.
- [ ] **B** — Jiný název nebo jiné rozdělení
  - Upřesni v poznámce – název ověřím na crates.io, GitHubu a doméně.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="2-realita-ambice-a-casovy-horizont"></a>

## 2. Realita, ambice a časový horizont

Tato sekce je nepříjemná, ale musí být první. Ověřená čísla z podzimu 2026: **Ladybird** má po 7,3 letech ~1,19 milionu řádků jádra (LibWeb ~830 tis., LibJS ~357 tis.), financování od Cloudflare, Shopify a FUTO – a jeho alpha stále není vydaná; Speedometer 3 má ~5,9. **Servo** má ~544 tis. řádků Rustu, 14,7 roku vývoje a 30 maintainerů, z toho jednoho částečně placeného. **Blitz** – nejbližší analog naší architektury (Stylo + Taffy + Parley + Vello) – má po 2,5 letech seriózní práce ~68–80 tis. řádků, grant od NLnet, a **stále nemá JavaScript**, `overflow: auto`, `position: sticky` ani skutečné tabulky (emuluje je gridem).

To neznamená, že to nemá smysl. Znamená to, že v0.1 musí být **ostře vymezený řez**, ne „všechno". Tato sekce určuje, jak hluboký řez to bude, kolik na něj máme času a co se stane, když se nestíhá.

### Q07 — Jakou úroveň ambice má v0.1 mít?

*vyber jednu*

**Proč to řešíme:** Toto je nejdůležitější otázka celého briefingu. Určuje, zda je 0.1 za 6 měsíců, za rok, nebo za dva – a co v ní bude chybět. „Použitelné jádro" je doporučená střední cesta: měřitelný výsledek, který není polotovar.

- [ ] **A** — Statické jádro (≈ 6–8 měsíců, 50–70 tis. řádků)
  - Vykreslí kurátorovaný korpus statických webů: HTML, CSS, blokový a inline layout, flex, obrázky, fonty, scroll. Bez JavaScriptu, formulářů, iframe a skutečných tabulek. Přesně velikost, na které je Blitz po 2,5 letech.
- [ ] **B** — Statické jádro + JS (≈ 12 měsíců, 120–160 tis. řádků) — ✅ **doporučeno**
  - Navíc: V8 + DOM bindingy + event loop + fetch + základní formuláře + timing. Stále bez Shadow DOM, iframe, plných tabulek a bidi. Toto je nejbližší realistická podoba „použitelného jádra" a doporučená varianta.
- [x] **C** — Browser-grade jádro (24+ měsíců, 250–350 tis. řádků) · **zvoleno**
  - Navíc: tabulky podle specifikace, floaty, bidi, Shadow DOM, navigace a historie, multi-proces, média, DevTools, přístupnost. Reálné, ale je to jiný projekt a jiný plán.
- [ ] **jiné** — ______________________

**Zvoleno:** C · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q08 — Jaký je tvrdý časový horizont do 0.1.0?

*vyber jednu*

**Proč to řešíme:** Horizont určuje, kolik funkcí se stihne a jak agresivně se musí řezat. Bez data se projekt nikdy „nedokončí", protože vždy půjde přidat další standard.

- [ ] **A** — 6 měsíců – jen statické vykreslování
  - Odpovídá variantě A v Q07. Stihne se vizuálně působivé demo, ale bez JS to není jádro pro weby, které dnes lidé používají.
- [ ] **B** — 9 měsíců – statické jádro + začátek JS
  - Reálně dosažitelné jen s tvrdým řezem: JS bindingy hotové, ale bez formulářů a bez tabulek. Vhodné, pokud chceš ukázat výsledek dřív a zbytek dodělávat za pochodu.
- [ ] **C** — 12 měsíců – statické jádro + JS (doporučeno) — ✅ **doporučeno**
  - Odpovídá variantě B v Q07. Zahrnuje jednu vlnu „skoro hotovo" a jednu vlnu stabilizace, testování a dokumentace.
- [x] **D** — 18–24 měsíců – browser-grade · **zvoleno**
  - Odpovídá variantě C v Q07. Rozsah jako malý prohlížeč, ne jen jádro. Nejvyšší kvalita, nejdelší cesta k prvnímu výsledku.
- [ ] **jiné** — ______________________

**Zvoleno:** D · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q07b — Jak dosáhnout cíle při maximálním rozsahu?

*vyber jednu*

**Proč to řešíme:** Součet voleb (vlastní rasterizér, OOPIF, multi-proces, video, WebGL/WebGPU, WebRTC, Service Workers, VoiceOver) odpovídá rozsahu celého prohlížeče — realisticky 400–450 tis. řádků. Čtyři agenti to za 24 měsíců nezvládnou; tato otázka rozhoduje, čím to vyřešíme.

- [ ] **A** — Víc agentů (8–12) + paralelní moduly
  - Zkrácení na ~24–30 měsíců; moduly jsou oddělitelné. Lead se stává úzkým hrdlem review.
- [x] **B** — Prodloužit horizont na 36–48 měsíců — ✅ **doporučeno** · **zvoleno**
  - Zachová rozsah i kvalitu; první demo později, ale bez přehlcení týmu. Doporučeno.
- [ ] **C** — Ponechat 24 měsíců, těžké API do 0.5
  - WebRTC, Service Workers, video, WebGL a VoiceOver až v navazující verzi.
- [ ] **D** — Rozhodnout po M2 podle naměřené rychlosti
  - Odložené rozhodnutí; riskantní pro plánování a nábor agentů.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q09 — Souhlasíš s tímto členěním na milníky M0–M5?

*vyber jednu*

**Proč to řešíme:** Milníky jsou veřejný závazek a zároveň nástroj řízení. Každý má být demonstrovatelný (screenshot, benchmark, test) – ne „hotovo v hlavě".

- [x] **A** — Ano, M0–M5 jak jsou navrženy — ✅ **doporučeno** · **zvoleno**
  - M0 (týdny 0–2): repo, CI, workspace, dokumentace, ADR, první trojúhelník přes vello. M1 (3–8): HTML→DOM→CSS→blokový/inline layout→text, screenshot a WPT harness od prvního týdne. M2 (9–16): flex, obrázky, barvy, scroll, hit-testing, vrstvy, stacking contexts. M3 (17–24): V8 + DOM bindingy + fetch + event loop + formuláře, interaktivní appky. M4 (25–34): korpus, inkrementální invalidace, výkon, paměť, benchmark brány. M5 (35–44): dokumentace EN+CS, 0.1.0, demo, blog.
- [ ] **B** — Míň milníků (3–4)
  - Jednodušší řízení, ale delší doba bez viditelného výsledku a větší riziko, že se chyba objeví pozdě.
- [ ] **C** — Víc, jemnějších milníků (8–10)
  - Lepší přehled a častější dema, ale víc režie a riziko, že se milníky stanou administrativou.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q10 — Co se má řezat jako první, když se nestíhá?

*vyber jednu*

**Proč to řešíme:** Rozhodnout se předem je mnohem levnější než rozhodovat ve stresu. Doporučené pořadí je od nejméně používaného k nejdůležitějšímu – tak, aby každý řez ublížil co nejméně reálným webům.

- [x] **A** — Předem odsouhlasené pořadí: tabulky → floaty → iframes → Shadow DOM → animace → workers → Wasm — ✅ **doporučeno** · **zvoleno**
  - Tabulky a floaty jsou vzácné na moderních webech; animace a workers se dají dohnat po 0.1. Wasm je drahý a pro Speedometer nepotřebný.
- [ ] **B** — Rozhodovat ad hoc podle situace
  - Flexibilnější, ale v praxi to znamená, že se řeže to, co zrovna bolí, ne to, co je nejméně důležité.
- [ ] **C** — Neřezat, prodloužit termín
  - Nejlepší kvalita, ale hrozí, že se termín posune opakovaně a projekt ztratí tah.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q11 — Šířka, nebo hloubka?

*vyber jednu*

**Proč to řešíme:** U omezeného času jde buď pokrýt víc webů „dost dobře", nebo méně webů „perfektně". Tato volba se promítá do každého rozhodnutí o CSS a layoutu.

- [x] **A** — Šířka: víc webů vypadá správně, i s drobnými chybami — ✅ **doporučeno** · **zvoleno**
  - Lepší dojem při procházení korpusu a lepší příběh („funguje na 70 z top-100"). Vyžaduje tolerantní fallbacky v CSS a layoutu.
- [ ] **B** — Hloubka: méně webů perfektně, zbytek odmítnout
  - Lepší technická kvalita a čistší kód, ale veřejnost to bude vnímat jako nefunkční jádro.
- [ ] **C** — Vyváženě podle korpusu z Q02
  - Rozumné, ale v praxi to znamená explicitní seznam „must pass" a „best effort" stránek – což je vlastně varianta A s lepší dokumentací.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="3-licence-verejnost-a-podoba-repozitare"></a>

## 3. Licence, veřejnost a podoba repozitáře

Projekt má být od začátku veřejný, aktivní a použitelný jinými vývojáři. To není jen otázka marketingu – licence a podoba repozitáře ovlivňují, jaké knihovny smíme použít a jak snadno nám někdo přispěje.

Pozor na jednu věc: část klíčových knihoven z ekosystému Servo/Mozilla je **MPL-2.0** – konkrétně `cssparser`, `selectors`, `stylo*`, `webrender`, `mozjs` a `lightningcss`. Naopak `html5ever` je MIT/Apache-2.0. MPL je permisivní licence kompatibilní s MIT/Apache i s GPL, ale vyžaduje, aby upravené soubory zůstaly pod MPL a byly zveřejněné. Pokud je pouze používáme jako závislosti, náš kód pod MIT/Apache může zůstat uzavřenější – což je pro adopci výhodné. Pozor: **čistě permisivní CSS kaskáda v Rustu neexistuje**, takže volba licence je zároveň volbou závislostí.

### Q12 — Jakou licenci bude mít náš kód?

*vyber jednu*

**Proč to řešíme:** Licence určuje, kdo nás smí použít – od studenta po firmu. Dual MIT/Apache-2.0 je standard Rust ekosystému a maximalizuje adopci i kompatibilitu s MPL závislostmi.

- [x] **A** — MIT OR Apache-2.0 (dual) — ✅ **doporučeno** · **zvoleno**
  - Standard Rustu. Firmy preferují Apache-2.0 (patentová doložka), jednotlivci MIT. Kompatibilní s MPL-2.0 závislostmi. Doporučeno.
- [ ] **B** — MPL-2.0
  - Stejná licence jako Servo/Firefox. Copyleft na úrovni souborů: kdokoli, kdo upraví náš soubor, musí změnu zveřejnit. Chrání projekt před uzavřenými forky, ale odrazuje část firemních uživatelů.
- [ ] **C** — GPL-3.0
  - Silný copyleft na celý binární celek. Znamená, že každý prohlížeč postavený na našem jádře musí být také GPL. Výrazně omezí adopci.
- [ ] **D** — AGPL-3.0
  - Jako GPL i pro síťové použití. Pro jádro prohlížeče nepraktické a pro většinu firem nepoužitelné.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q13 — Kdy bude repozitář veřejný?

*vyber jednu*

**Proč to řešíme:** Chceš, aby bylo vidět, že projekt žije. Veřejnost od začátku je nejlepší důkaz, ale znamená to disciplínu v commitech a dokumentaci od prvního dne – což je přesně to, co stejně chceme.

- [x] **A** — Veřejný od prvního commitu, s devlogem — ✅ **doporučeno** · **zvoleno**
  - Každý týden krátký devlog (co, proč, co dál) + roadmapa. Nejsilnější signál „projekt žije". Riziko: veřejnost vidí i experimenty a slepé uličky – řeší se to poctivým devlogem.
- [ ] **B** — Veřejný od M1 (první vykreslená stránka)
  - Nevystavujeme prázdný repozitář. O pár týdnů později, ale první dojem je silnější.
- [ ] **C** — Soukromý až do 0.1.0
  - Nejmenší tlak, ale ztrácíme hlavní výhodu – viditelnou aktivitu a možnost získat přispěvatele brzy.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q14 — Jak bude vypadat „co se změnilo" u každého commitu?

*vyber jednu*

**Proč to řešíme:** Chceš, aby každý commit říkal, co se změnilo. Standardní cesta je Conventional Commits + generovaný CHANGELOG + PR šablona, která vynutí kontext. Ruční changelog u každého commitu je nerealistický a v praxi se přestane dodržovat.

- [x] **A** — Conventional Commits + auto CHANGELOG + PR šablona (Co/Proč/Dopad/Měření) — ✅ **doporučeno** · **zvoleno**
  - Commit message nese typ a rozsah, CHANGELOG se generuje při release, PR šablona vynutí vysvětlení dopadu a naměřených čísel. Nejlepší poměr přínosu a úsilí.
- [ ] **B** — Ruční detailní changelog u každého commitu
  - Nejvíc informací, ale vysoká režie a rychle se přestane dodržovat. Vhodné jen pro velmi malý počet commitů.
- [ ] **C** — Squash merge s bohatým popisem PR
  - Dobrá historie na úrovni PR, ale jednotlivé commity během práce jsou neinformativní.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q15 — Které „signály života" a governance soubory chceme od začátku?

*vyber vše, co platí*

**Proč to řešíme:** Toto je to, co lidé u nového projektu kontrolují, než mu věnují čas. Většina je hotová za hodinu a vydrží roky.

- [x] **A** — CONTRIBUTING.md + vývojářská příručka — ✅ **doporučeno** · **zvoleno**
  - Jak postavit, jak testovat, jak poslat PR, jaké jsou konvence.
- [x] **B** — CODE_OF_CONDUCT.md — ✅ **doporučeno** · **zvoleno**
  - Standard (Contributor Covenant). Levné, očekávané.
- [x] **C** — SECURITY.md + způsob hlášení zranitelností — ✅ **doporučeno** · **zvoleno**
  - Pro prohlížečové jádro nutnost – budeme zpracovávat nedůvěryhodný obsah.
- [x] **D** — Šablony issue a PR + návrh roadmapy (GitHub Projects) — ✅ **doporučeno** · **zvoleno**
  - Veřejná roadmapa je nejsilnější signál, že projekt má směr.
- [x] **E** — GitHub Discussions · **zvoleno**
  - Prostor pro otázky a návrhy; užitečné, ale chce moderaci.
- [x] **F** — Týdenní devlog (blog/Discussions) a štítky stavu v README — ✅ **doporučeno** · **zvoleno**
  - Ukazuje pokrok i tam, kde zrovna není vidět kód.
- [x] **G** — Automatické release notes a semver kontrola — ✅ **doporučeno** · **zvoleno**
  - Release Please / release-plz. Dobré pro 0.x s rychlými iteracemi.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F, G · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

---

<a id="4-architektura-a-hranice-systemu"></a>

## 4. Architektura a hranice systému

Tato sekce určuje tvar kódu na několik let. Špatné rozhodnutí tady se nedá dodatečně opravit bez přepsání celého jádra – proto ho řešíme dřív než první řádek kódu.

Klíčový princip: **hranice musí být od začátku připravené na budoucnost, i když je implementace v 0.1 jednoduchá.** Typicky: multi-procesní izolace nemusí být hotová, ale komunikační rozhraní ano.

### Q16 — Jak má vypadat Cargo workspace?

*vyber jednu*

**Proč to řešíme:** Rozdělení na crates vynucuje hranice (nedá se snadno obejít API), zrychluje inkrementální build a umožňuje paralelní práci agentů bez konfliktů v jednom souboru.

- [x] **A** — Jeden workspace, jeden crate na vrstvu — ✅ **doporučeno** · **zvoleno**
  - např. jkva-core, jkva-dom, jkva-style, jkva-layout, jkva-paint, jkva-text, jkva-net, jkva-platform, jkva-js, jkva-shell. Čisté hranice, rychlý build, snadné publikování na crates.io.
- [ ] **B** — Monolitický crate
  - Nejrychlejší start, ale po pár měsících neúnosné: dlouhé buildy, žádné vynucené hranice, konflikty při paralelní práci.
- [ ] **C** — Vnořené workspaces (jádro / shell / nástroje)
  - Užitečné u velkých projektů; pro 0.1 zbytečná režie.
- [ ] **D** — Jeden workspace + samostatný „contract" crate s trait rozhraními
  - Varianta A rozšířená o explicitní kontrakty mezi vrstvami. Nejpřesnější hranice, ale víc boilerplate.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q17 — Jaký procesní model má mít 0.1?

*vyber jednu*

**Proč to řešíme:** Multi-proces je základ bezpečnosti moderních prohlížečů, ale je to obrovská režie (IPC, sdílená paměť, životní cyklus). Rozhodnutí určuje, zda nás bezpečnostní kritika smete hned, nebo až později.

- [ ] **A** — Jeden proces, více vláken, ale IPC-připravené hranice — ✅ **doporučeno**
  - Všechny přechody mezi síťí, rendererem a GPU jdou přes message-passing rozhraní, i když běží ve stejném procesu. V 0.2+ lze vyměnit transport za skutečné procesy bez přepsání logiky.
- [x] **B** — Skutečný multi-proces od začátku · **zvoleno**
  - Nejbezpečnější, ale od prvního dne řešíme sandbox, sdílenou paměť, pády a ladění. Realisticky +6 měsíců a výrazně vyšší náročnost.
- [ ] **C** — Hybrid: samostatné procesy pro síť a GPU, renderer zatím jeden
  - Kompromis: síť a GPU jsou přirozeně izolované a snadno se oddělí, renderer zůstává jednoduchý.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q18 — Jaký má být model vláken a plánování?

*vyber jednu*

**Proč to řešíme:** Apple Silicon má P-core a E-core s velmi rozdílnými vlastnostmi. Bez explicitního plánování skončí náročná práce na efektivních jádrech a UI trhá. Zároveň příliš mnoho vlastních vláken škodí.

- [x] **A** — Actor/message-passing s explicitními prioritami a QoS — ✅ **doporučeno** · **zvoleno**
  - Každá vrstva má vlastní doménu a komunikuje zprávami. Priority: Interactive (scroll, input, paint), UserInitiated (layout, JS), Utility (decode, cache), Background (index, GC). Mapuje se na QoS třídy macOS.
- [ ] **B** — Vše na tokio, async/await
  - Skvělé pro síť a I/O, ale layout a vykreslování jsou CPU-bound a synchronní – async model jim nesedí a komplikuje determinismus testů.
- [ ] **C** — Vlákno na každou starost + kanály
  - Jednoduché a předvídatelné, ale špatně se škáluje a snadno vznikne 20 vláken s neznámou prioritou.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q19 — Kdo vlastní event loop?

*vyber jednu*

**Proč to řešíme:** Vlastnictví smyčky určuje, jak snadno jde jádro vložit do cizí aplikace a jak testovat deterministicky. Embedder-driven model (jako Servo) je flexibilnější a lépe se testuje.

- [x] **A** — Embedder řídí smyčku (pump/tick), jádro má vnitřní async — ✅ **doporučeno** · **zvoleno**
  - Shell nebo test zavolá pump() v přesně daných okamžicích. Deterministické testy, snadné vložení do SwiftUI/AppKit aplikace, žádné skryté vlákno.
- [ ] **B** — Jádro vlastní smyčku a spouští ji samo
  - Jednodušší pro samostatný shell, ale obtížně se vkládá do cizí aplikace a testuje.
- [ ] **C** — Plně async s waker-driven exekutorem
  - Moderní, ale pro prohlížečové jádro se složitou fází životního cyklu dokumentu přináší mnoho okrajových stavů.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q20 — Jaké rozhraní pro vkládání (embedding) bude jádro mít?

*vyber jednu*

**Proč to řešíme:** Veřejné API je to, co budou používat jiní vývojáři. Špatně navržené API zamrzne na roky. Rust API je přirozené, ale C ABI otevírá dveře Swiftu, C++ a dalším.

- [ ] **A** — Rust API (trait objekty + handle typy), C ABI až po 0.1 — ✅ **doporučeno**
  - Rychlý vývoj, idiomatické API, žádné unsafe. C ABI se doplní, až bude API stabilní – jinak bychom ho museli měnit dvakrát.
- [ ] **B** — C ABI od začátku
  - Okamžitě použitelné ze Swifta, C++ i Pythonu, ale návrh v C je pomalejší a méně bezpečný; každá změna API bolí.
- [x] **C** — C++ API (jako Blink/WebKit) · **zvoleno**
  - Nejblíž existujícím prohlížečům, ale ztrácíme hlavní výhodu Rustu – bezpečnost na hranicích.
- [ ] **D** — Rust API + stabilní C ABI pro malou množinu operací (okno, vstup, životní cyklus)
  - Pragmatické: jádro zůstává rustové, ale shell může být nativní. Víc práce než A, ale otevírá iOS/macOS integraci.
- [ ] **jiné** — ______________________

**Zvoleno:** C · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q21 — Jak bude vypadat DOM v paměti?

*vyber jednu*

**Proč to řešíme:** DOM je nejčastěji přistupovaná struktura v celém jádře. Volba ovlivňuje výkon, bezpečnost i složitost JS bindingů. Rc<RefCell> je pohodlné, ale pomalé a náchylné k panikám; arena je rychlá a předvídatelná.

- [x] **A** — Generační arena + typované indexy (NodeId s generací) — ✅ **doporučeno** · **zvoleno**
  - Uzly v souvislém bloku, přístup O(1), výborná cache lokalita, snadné uvolnění celé stránky najednou. Generace odhalí use-after-free v debug buildu. Nutné pro výkon a bezpečnost.
- [ ] **B** — Rc<RefCell<Node>>
  - Pohodlné a flexibilní, ale každý přístup má runtime kontrolu, cykly unikají a výkon je výrazně horší.
- [ ] **C** — Raw pointery se unsafe (styl Chromium)
  - Maximální kontrola, ale přesouváme celou bezpečnost na disciplínu vývojářů – přesně to, čemu se chceme vyhnout.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="5-apple-silicon-gpu-a-vlakna"></a>

## 5. Apple Silicon, GPU a vlákna

Tohle je místo, kde můžeme být objektivně lepší než konkurence. Apple Silicon má unifikovanou paměť (CPU a GPU vidí stejná data), výkonné P-core a úsporné E-core a Metal. Když tomu přizpůsobíme architekturu, získáme výhodu, kterou Chromium ani Gecko jen tak nedoženou – jejich architektura vznikala pro diskrétní GPU a x86.

Pozor ale: příliš brzká optimalizace na Metal nás zamkne. Proto jsou otázky níže o tom, kde je integrace přínosem a kde jen rizikem.

### Q22 — Přes jaké API mluvit s GPU?

*vyber jednu*

**Proč to řešíme:** Metal přímo = nejnižší režie a plná kontrola, ale kód je nepřenosný a musíme psát vlastní abstrakci. wgpu = přenositelnost, bezpečnější API, aktivní vývoj, ale další vrstva a občas chybějící funkce.

- [ ] **A** — wgpu (Metal backend) s dokumentovanou únikovou cestou k přímému Metal — ✅ **doporučeno**
  - Přenositelné (Linux CI, budoucí Windows/iOS), bezpečné Rust API, WGSL shadery. wgpu 30 je Metal backendem pro Firefox, Servo i Deno, takže cesta je ověřená. Pozor: wgpu vydává breaking verzi každé čtvrtletí – počítej s ~týdnem údržby na major verzi a drž ho za tenkou vlastní abstrakcí. Pro horká místa lze později přidat přímý Metal bez přepsání zbytku.
- [x] **B** — Přímý Metal přes objc2-metal · **zvoleno**
  - Maximální výkon a kontrola (argument buffers, residency sets, MetalFX). Mnohem víc unsafe kódu a žádná přenositelnost – CI na Linuxu bez GPU.
- [ ] **C** — Vlastní tenká abstrakce nad Metalem + CPU fallback
  - Nejlepší výkon i přenositelnost, ale abstrakci si musíme napsat a udržovat sami. Realistické až po 0.1.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q22b — Jak sladit přímý Metal s rasterizérem?

*vyber jednu*

**Proč to řešíme:** Q22 zvolil přímý Metal 4, ale Q23 počítal s vellem, které běží na wgpu. Nelze mít obojí bez explicitního rozhodnutí — a je to levné rozhodnout teď, drahé později.

- [ ] **A** — Metal 4 pro kompozici + vello přes wgpu
  - Dvě GPU cesty vedle sebe; nejsložitější synchronizace.
- [ ] **B** — Metal 4 + vello_cpu (CPU raster)
  - Jedna cesta bez wgpu, ale CPU raster je pro 120 Hz a velké stránky riziko.
- [x] **C** — Metal 4 + vlastní GPU rasterizér — ✅ **doporučeno** · **zvoleno**
  - Nejvýkonnější a nejčistší; rasterizér cest, AA a glyfů je měsíce práce navíc. Doporučeno.
- [ ] **D** — Přejít na wgpu + vello_gpu
  - Jedna cesta, nejrychlejší vývoj; Metal 4 featurury přijdou později.

**Zvoleno:** C · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q23 — Čím rasterizovat vektory a text?

*vyber jednu*

**Proč to řešíme:** Tohle rozhoduje o vizuální kvalitě i výkonu. Vello prošlo v roce 2026 restrukturalizací: `vello` 0.11 je experimentální compute renderer, produkční jsou `vello_gpu` 0.3 (Sparse Strips, běží na wgpu/Metal) a `vello_cpu` 0.3 (nejzralejší, ideální pro CI). Vlastní rasterizér je měsíce práce s nejistým výsledkem. **Pozor:** rozhodnutí Q22b tuto volbu nahrazuje — rasterizér bude vlastní, vello zůstává jen jako referenční srovnání pro testy.

- [x] **A** — vello_gpu pro cesty a glyfy + vello_cpu jako fallback pro CI/headless — ✅ **doporučeno** · **zvoleno**
  - Nejrychlejší cesta k „krásnému" vykreslování: kvalitní AA, GPU akcelerace, aktivní vývoj. `vello_gpu` má nativní GPU rasterizaci glyfů (`Scene::glyph_run`) a atlas; `vello_cpu` dá deterministické screenshot testy bez GPU. Pozor na vazbu na verze wgpu.
- [ ] **B** — Vlastní GPU rasterizér
  - Plná kontrola a potenciálně nejlepší výkon, ale je to výzkumný projekt sám o sobě (roky) a odvádí nás od jádra.
- [ ] **C** — Nejdřív CPU (vello_cpu), GPU až později
  - Nejjednodušší start a snadné testování, ale plynulost 120 Hz a velké stránky na CPU nezvládneme – hrozí, že přepis bude dražší než začít na GPU.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q24 — Budeme explicitně řídit QoS a P/E jádra?

*vyber jednu*

**Proč to řešíme:** macOS umí dát vláknu QoS třídu (user-interactive, user-initiated, utility, background). Pokud to neuděláme, systém může naplánovat scroll na E-core a nejde za to. Je to levné a měřitelné.

- [x] **A** — Ano, každá doména má přiřazenou QoS třídu a prioritu — ✅ **doporučeno** · **zvoleno**
  - Input/scroll/paint = user-interactive; layout a JS = user-initiated; decode a cache = utility; indexace a úklid = background. Měřitelné zlepšení latence a spotřeby.
- [ ] **B** — Nechat rozhodnutí na OS
  - Jednodušší, ale nepředvídatelné – přesně to, na co si uživatelé stěžují u konkurence.
- [ ] **C** — Vlastní thread pool s affinitou
  - Na macOS nemá smysl: affinity není veřejné API a QoS je správný nástroj.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q25 — Kolik „zero-copy" UMA integrace chceme už v 0.1?

*vyber jednu*

**Proč to řešíme:** Sdílená paměť CPU/GPU je největší architektonická výhoda Apple Silicon: data z layoutu mohou jít do GPU bez kopie. Je ale snadné na tom uvíznout – plná zero-copy pipeline je náročná na synchronizaci.

- [ ] **A** — Navrhnout buffery jako sdílené od začátku, implementovat zero-copy tam, kde je to zdarma — ✅ **doporučeno**
  - Display list, glyph atlas a velké obrázky půjdou do sdílené paměti; zbytek může kopírovat. Architektura zůstane otevřená, rozsah přiměřený.
- [x] **B** — Plná zero-copy pipeline v 0.1 · **zvoleno**
  - Nejlepší výkon a paměť, ale synchronizace CPU/GPU a životnost bufferů jsou složité; hrozí, že se zdrží celý M2.
- [ ] **C** — Řešit až po 0.1
  - Jednodušší, ale později to znamená přepis paměťového modelu vykreslování.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q26 — Jaké platformy cílíme v 0.1?

*vyber jednu*

**Proč to řešíme:** Každá platforma násobí testovací matici a odhalí jiné chyby. Zároveň potřebujeme levné CI – a to je Linux. Rozumný kompromis: produktově jen macOS arm64, ale jádro nesmí být zamčené na macOS.

- [x] **A** — macOS arm64 jako jediný produktový cíl; jádro musí jít zkompilovat a testovat na Linuxu (CI, headless) — ✅ **doporučeno** · **zvoleno**
  - Dostaneme levné a rychlé CI, deterministické screenshot testy bez GPU a přípravu na budoucí porty. Produktově se soustředíme na Mac.
- [ ] **B** — macOS arm64 + iOS hned
  - Dva shelly, dvě sady omezení (sandbox, paměť, žádný JIT v některých režimech). Realisticky +4–6 měsíců.
- [ ] **C** — macOS arm64 + macOS x86_64 (Rosetta i nativně)
  - Intel Macy jsou na ústupu; udržovat dvě architektury kvůli malé skupině uživatelů nedává smysl.
- [ ] **D** — macOS + Linux + Windows od začátku
  - Přenositelné jádro je hezké, ale produktová kvalita na třech platformách v jednom roce není reálná.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="6-pametova-strategie"></a>

## 6. Paměťová strategie

Paměť je náš hlavní slib (Q03/Q04). Chromium je známé tím, že „sežere" RAM s mnoha kartami; Safari je na Macu výrazně úspornější, ale ne dokonalé. Rust nám dává nástroje, které C++ nemá: areny, striktní vlastnictví a absenci skrytých alokací.

Pozor na měření: „spotřeba paměti" není jedno číslo. Rozlišujeme RSS (rezidentní paměť), footprint (fyzicky použité stránky), dirty pages a špičkovou alokaci. V briefingu budeme vždy uvádět, kterou metriku myslíme.

### Q27 — Jaký globální alokátor?

*vyber jednu*

**Proč to řešíme:** Globální alokátor ovlivňuje fragmentaci, rychlost přidělování i to, jak moc „nafouknutá" paměť vypadá v Activity Monitoru. U jádra s arenami je jeho role menší, ale pořád rozhoduje o špičkách.

- [x] **A** — Systémový malloc (Apple) — ✅ **doporučeno** · **zvoleno**
  - Nejlepší spolupráce s macOS (footprint, komprese paměti, malloc zóny), žádná další závislost. Dobrý výchozí bod, když většinu práce stejně dělají areny.
- [ ] **B** — mimalloc
  - Rychlý, nízká fragmentace, dobré výsledky v serverech. Na macOS může hůř sedět s kompresí paměti a měřením footprintu.
- [ ] **C** — jemalloc
  - Kvalitní a předvídatelný, ale velký a na macOS spíš neobvyklý; výhody se projeví hlavně u fragmentace dlouho běžících procesů.
- [ ] **D** — Vlastní alokátor s arenami jako primární strategií
  - Nejlepší kontrola a nejnižší režie, ale je to samostatný projekt s vysokým rizikem chyb. Vhodné až po 0.1.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q28 — Jak budou fungovat areny?

*vyber jednu*

**Proč to řešíme:** Arena je největší paměťová výhra: místo tisíce malých alokací uděláme jednu velkou a celou ji uvolníme najednou. U prohlížeče to sedí na životní cyklus dokumentu a snímku.

- [x] **A** — Arena na dokument (reset při navigaci) + efemérní arena na snímek (layout/paint) — ✅ **doporučeno** · **zvoleno**
  - Dokument drží DOM, styl a layout strom; snímková arena drží dočasné výpočty a display list. Uvolnění je O(1), fragmentace minimální.
- [ ] **B** — Jen arena na dokument, zbytek běžné alokace
  - Jednodušší, ale snímkové alokace (layout, display list) budou tvořit špičky a fragmentaci.
- [ ] **C** — Bez aren, spoléhat na Rc/Box
  - Nejjednodušší, ale přesně tímto způsobem vzniká „Chrome žere RAM".
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q29 — Budeme mít tvrdý paměťový rozpočet na dokument?

*vyber jednu*

**Proč to řešíme:** Bez limitu roste paměť s každou otevřenou stránkou a špatně napsaným webem. Soft cap s evikcí (obrázky, cache, bfcache) udrží jádro předvídatelné a je to přímý důkaz našeho slibu.

- [x] **A** — Soft cap na dokument + evikce + diagnostika — ✅ **doporučeno** · **zvoleno**
  - např. 256 MB na dokument: při překročení se uvolní dekódované obrázky, bfcache a cache, ale stránka nespadne. Metriky se hlásí do dev overlaye a benchmarků.
- [ ] **B** — Bez limitu
  - Jednodušší, ale neumíme garantovat nic a benchmarky paměti budou nepředvídatelné.
- [ ] **C** — Tvrdý limit s ukončením dokumentu
  - Rozhodné, ale uživatelsky nepříjemné (bílá stránka) a těžko se ladí.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q30 — Které cache a zálohy chceme už v 0.1?

*vyber jednu*

**Proč to řešíme:** bfcache (rychlý návrat zpět) výrazně zlepšuje vnímanou rychlost, ale drží celé dokumenty v paměti – jde proti našemu slibu. Obrázková cache je naopak nutná pro plynulost. Rozhodnutí musí být explicitní.

- [x] **A** — bfcache pro 1–2 dokumenty + rozpočet na dekódované obrázky — ✅ **doporučeno** · **zvoleno**
  - Zpět/vpřed je okamžité, paměť zůstává pod kontrolou díky capu z Q29. Obrázky se drží dekódované v limitu a jinak se re-dekódují.
- [ ] **B** — Jen obrázková cache, bfcache až po 0.1
  - Nejnižší paměť, ale zpět/vpřed bude pomalé a uživatelsky znát.
- [ ] **C** — Žádné cache
  - Nejnižší paměť a nejjednodušší, ale plynulost a vnímaná rychlost budou špatné.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="7-sitova-vrstva"></a>

## 7. Síťová vrstva

Síť je první věc, kterou uživatel pocítí: čím dřív dorazí HTML a první kritické soubory, tím dřív je vidět stránka. Zároveň je to vrstva, kde se nejvíc projevuje integrace s platformou (proxy, trust store, HTTP/3) a kde se dají snadno udělat bezpečnostní chyby.

V 0.1 nepotřebujeme vlastní TCP stack. Potřebujeme dobře zvolenou knihovnu, správnou cache a čisté rozhraní, které půjde později přesunout do samostatného procesu.

### Q31 — Na čem postavit HTTP?

*vyber jednu*

**Proč to řešíme:** Volba určuje výkon, kontrolu nad cache a hlavně přenositelnost. NSURLSession je na Macu nejlépe integrovaná (proxy, HTTP/3, trust store, spořič energie), ale nedává nám přístup k detailům, které potřebujeme pro cache a měření.

- [x] **A** — hyper + rustls + tokio — ✅ **doporučeno** · **zvoleno**
  - Plná kontrola nad požadavky, hlavičkami a cache; přenositelné na Linux CI; čistě Rust. Musíme si sami dořešit proxy, trust store a HTTP/3.
- [ ] **B** — NSURLSession přes FFI
  - Nejlepší integrace s macOS: systémová proxy, HTTP/3, trust store, energetická optimalizace. Ale omezená kontrola, horší testovatelnost a vazba na Apple platformy.
- [ ] **C** — libcurl
  - Zralé a všestranné, ale C API, vlastní model vláken a horší integrace s async Rustem.
- [ ] **D** — hyper jako základ + volitelný „system transport" pro macOS
  - Abstrakce nad transportem: výchozí hyper, na macOS lze přepnout na NSURLSession pro systémy s proxy/MITM certifikáty. Víc práce, ale nejlepší z obou světů.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q32 — Odkud brát důvěryhodné certifikáty?

*vyber jednu*

**Proč to řešíme:** Vlastní seznam kořenových certifikátů znamená, že weby za firemní proxy nebo s podnikovým MITM certifikátem neotevřeme. Uživatelé to vnímají jako „prohlížeč je rozbitý".

- [x] **A** — rustls + rustls-platform-verifier (systémový trust store) — ✅ **doporučeno** · **zvoleno**
  - Respektuje certifikáty nainstalované v macOS, funguje za firemní proxy. Bezpečné a očekávané chování.
- [ ] **B** — Jen webpki-roots (zabudovaný seznam)
  - Deterministické a jednoduché, ale rozbije firemní prostředí a vývojové proxy.
- [ ] **C** — Vlastní správa certifikátů s UI
  - Maximální kontrola, ale musíme řešit expiraci, revokaci a uživatelské rozhraní – zbytečná práce.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q33 — HTTP/3 (QUIC) už v 0.1?

*vyber jednu*

**Proč to řešíme:** HTTP/3 zlepšuje latenci na mobilních a ztrátových sítích, ale přidává další transport, ladění a testovací matici. Pro desktop na stabilní síti je přínos menší než pro mobil.

- [ ] **A** — Ne v 0.1, ale architektura s ním počítá (alt-svc, výměna transportu) — ✅ **doporučeno**
  - HTTP/1.1 + HTTP/2 pokryjí drtivou většinu webů. QUIC přidáme v 0.2 – buď `quinn` (nejlepší Rust QUIC), nebo `neqo` (co používá Firefox) – bez zásahu do zbytku.
- [x] **B** — Ano, quinn hned v 0.1 · **zvoleno**
  - Moderní a dobře vypadá v benchmarkách, ale +1–2 měsíce práce a riziko nestability.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q34 — Jak řešit HTTP cache?

*vyber jednu*

**Proč to řešíme:** Cache je největší jednostranný zisk výkonu (a úspory paměti i sítě) a zároveň častý zdroj chyb (zastaralý obsah, špatné Vary, nekonzistence s cookies).

- [x] **A** — RFC 9111 přes http-cache-semantics + vlastní disk cache s limitem — ✅ **doporučeno** · **zvoleno**
  - Ověřená logika čerstvosti a validace, vlastní úložiště s kompresí a LRU. Vhodné i pro offline režim.
- [ ] **B** — Vlastní implementace cache od nuly
  - Plná kontrola, ale RFC 9111 má mnoho okrajových případů; snadno vzniknou chyby, které se těžko hledají.
- [ ] **C** — Jen paměťová cache bez disku
  - Jednoduché a bezpečné, ale ztrácíme výhodu mezi spuštěními a offline režim.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q35 — Cookies, proxy a DoH – v jakém rozsahu?

*vyber jednu*

**Proč to řešíme:** Cookies jsou nutné pro přihlášení, ale zároveň největší stopovací kanál. Partitioning (CHIPS) a SameSite jsou dnes standard a ovlivňují, jak se weby chovají. Proxy je nutnost v korporátním prostředí.

- [x] **A** — cookie_store jako základ + vlastní obal (SameSite, partitioning, CHIPS) + systémová proxy + DoH později — ✅ **doporučeno** · **zvoleno**
  - Rozumný rozsah: správné chování cookies včetně moderních ochran, funkční proxy z macOS nastavení, DoH až s privacy vrstvou. Pozor: `cookie_store` nemá od února 2026 commity a neumí CHIPS – počítej s vlastním obalem nebo forkem.
- [ ] **B** — Vlastní cookie jar od nuly včetně partitioningu
  - Nejlepší kontrola a bezpečnost, ale je to víc práce, než se zdá (expirace, domény, path, Secure, HttpOnly, SameSite, CHIPS).
- [ ] **C** — Minimalisticky: cookies bez partitioningu, proxy jen ručně
  - Nejrychlejší, ale weby se budou chovat jinak než v Chrome/Safari a privacy příběh bude slabý.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="8-html-dom-a-css"></a>

## 8. HTML, DOM a CSS

Tohle je „vstupní brána" jádra. Zde platí nejvíc to, co říkal výzkum: **parsery se nevyplatí psát od nuly.** HTML5 parsing je definovaný tisícovkami pravidel pro chybové stavy a jeho ruční implementace je měsíce práce s vysokou pravděpodobností chyb, které produkční prohlížeče nemají. Stejně tak CSS syntaxe s forward-compatible parsingem.

Naše „from scratch" je jinde: v DOM reprezentaci, kaskádě, výpočtu stylů, invalidaci a v tom, jak vše propojíme s layoutem a JS. Tam vzniká výkon a kvalita.

### Q36 — HTML parser: použít html5ever, nebo psát vlastní?

*vyber jednu*

**Proč to řešíme:** html5ever je spec-compliant parser z dílny Servo/Mozilla, prošel testy html5lib a je v produkci. Vlastní parser znamená znovuobjevení všech chybových stavů (implicitní uzavírání tagů, tabulky, select, foster parenting) – a to je přesně práce, kterou nikdo neocení.

- [x] **A** — html5ever — ✅ **doporučeno** · **zvoleno**
  - Spec-compliant, rychlý, bezpečný, aktivně udržovaný. Strom si stavíme sami přes TreeSink, takže DOM zůstává náš.
- [ ] **B** — Vlastní parser od nuly
  - Plná kontrola a možnost přidat rozšíření (source mapy, streaming), ale realisticky 3–6 měsíců práce a vysoké riziko chyb.
- [ ] **C** — lol_html (streaming rewriter) jako základ
  - Skvělé pro transformace, ale není to plnohodnotný DOM builder; narazíme na limity u tabulek a foster parentingu.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q37 — Potřebujeme pozice ve zdrojovém HTML (source spans) už v 0.1?

*vyber jednu*

**Proč to řešíme:** Source spans umožňují ukázat „tento prvek je na řádku 42" – nutné pro DevTools, error reporting a nástroje. html5ever je historicky neposkytuje a jejich doplnění znamená fork nebo vlastní vrstvu.

- [x] **A** — Ne, odložit na dobu, kdy budou DevTools — ✅ **doporučeno** · **zvoleno**
  - V 0.1 nemáme DevTools UI (Q05), takže spans nemají konzumenta. Ušetříme fork a údržbu.
- [ ] **B** — Ano, fork html5ever s podporou spans hned
  - Připraveno pro budoucnost a lepší diagnostiku, ale musíme udržovat fork a hlídat upstream změny.
- [ ] **C** — Lehká mapa spans jen pro chyby parseru
  - Kompromis: přesné pozice jen tam, kde hlásíme chybu, ne pro každý uzel.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q38 — Jak postavit DOM?

*vyber jednu*

**Proč to řešíme:** DOM je naše IP a nejčastěji používaná struktura. Převzetí cizího DOM (blitz-dom) zrychlí start, ale přineseme si cizí rozhodnutí a závislost. Vlastní DOM postavený na html5ever TreeSink je přirozená hranice.

- [x] **A** — Vlastní arena DOM napojený na html5ever TreeSink — ✅ **doporučeno** · **zvoleno**
  - Kontrola nad reprezentací, možnost optimalizovat pro layout i JS bindingy. Odpovídá Q21 (generační arena).
- [ ] **B** — Převzít blitz-dom a stavět na něm
  - Ušetří měsíce a je to nejblíž existujícímu řešení, ale dědíme jejich model, tempo vývoje a omezení; „from scratch" se rozpadá.
- [ ] **C** — Rc<RefCell> DOM
  - Nejrychlejší prototyp, ale výkonově i bezpečnostně slepá ulička.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q39 — CSS: kaskáda a výpočet stylů – postavit na čem?

*vyber jednu*

**Proč to řešíme:** Toto je největší jednotlivé rozhodnutí v celém jádře. Kaskáda, dědičnost, shorthandy, jednotky, barvy, media queries a invalidace jsou dohromady roky práce. Stylo (Firefox) je nejzralejší implementace, ale je pevně svázané s DOM a je to obrovská závislost.

- [x] **A** — lightningcss/cssparser pro parsování + vlastní kaskáda a computed values nad řízenou sadou vlastností — ✅ **doporučeno** · **zvoleno**
  - Moderní syntaxe (nesting, oklch, @layer) zdarma, kontrola nad kaskádou a invalidací, žádné svázání s cizím DOM. Rozsah vlastností řídíme daty o reálném využití – Blitz zveřejňuje tabulku procentuálního výskytu vlastností na reálných webech, což je ideální prioritizační dataset. V M0 navíc uděláme týdenní spike Stylo, abychom rozhodli podle dat, ne dojmu.
- [ ] **B** — Integrace Stylo (crate `stylo` z projektu Servo)
  - Nejvyšší správnost a roky optimalizací zdarma, včetně invalidace a animací. `stylo` 0.22 je publikované (2026-09) a Blitz ho reálně používá. Ale musíme implementovat Servo DOM traity, nést MPL-2.0 závislost a API se hýbe (0.19→0.22 za tři měsíce); Blitz kvůli tomu drží vlastní `stylo_taffy` glue crate.
- [ ] **C** — Vlastní parser i kaskáda od nuly
  - Maximální kontrola a „čistota", ale realisticky +9–12 měsíců a vysoké riziko chyb ve forward-compatible parsingu.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q40 — Které CSS funkce musí 0.1 umět?

*vyber vše, co platí*

**Proč to řešíme:** Tohle je definice „moderního webu". Moderní weby (Tailwind 4, komponentové knihovny) používají @layer, custom properties, oklch a nesting. Bez nich vypadají rozbitě, i když základní box model funguje.

- [x] **A** — Základ: box model, barvy, pozadí, okraje, fonty, text — ✅ **doporučeno** · **zvoleno**
  - Bez toho nefunguje nic.
- [x] **B** — Flexbox — ✅ **doporučeno** · **zvoleno**
  - Drtivá většina moderních layoutů.
- [x] **C** — Grid — ✅ **doporučeno** · **zvoleno**
  - Dashboardy, galerie, složité layouty; dnes standard.
- [x] **D** — Custom properties (CSS proměnné) — ✅ **doporučeno** · **zvoleno**
  - Základ témat a komponentových knihoven.
- [x] **E** — Cascade layers (@layer) — ✅ **doporučeno** · **zvoleno**
  - Tailwind 4 a moderní reset styly je používají; bez nich je kaskáda špatně.
- [x] **F** — Nesting (&) — ✅ **doporučeno** · **zvoleno**
  - Dnes běžné v ručně psaném i generovaném CSS.
- [x] **G** — :has(), :is(), :where(), :not() — ✅ **doporučeno** · **zvoleno**
  - Moderní selektory; :has je dnes široce používaný.
- [x] **H** — Container queries · **zvoleno**
  - Komponentové knihovny je začínají používat; náročné na invalidaci.
- [x] **I** — Transforms, transitions, animations — ✅ **doporučeno** · **zvoleno**
  - Bez transitions působí UI mrtvě; animations jsou potřeba pro loadery.
- [x] **J** — Barvy 4. úrovně (oklch, color-mix, P3) — ✅ **doporučeno** · **zvoleno**
  - Tailwind 4 a moderní design systémy; zároveň příležitost pro náš „krásný" příběh.
- [x] **K** — Gradients, shadows, filters, clip-path, mask — ✅ **doporučeno** · **zvoleno**
  - Vizuální kvalita a moderní UI; některé (backdrop-filter) jsou drahé.
- [x] **L** — Scroll-driven animations a view transitions · **zvoleno**
  - Efektní, ale nové a náročné; vhodné až po 0.1.
- [x] **M** — Anchor positioning · **zvoleno**
  - Velmi nové; odložit.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F, G, H, I, J, K, L, M · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q41 — Selektorový engine: vlastní, nebo hotový?

*vyber jednu*

**Proč to řešíme:** Selektory jsou překvapivě složité (specificita, :nth-child s of, :has, shadow scoping) a zároveň kritické pro výkon stylování. `selectors` crate je produkční implementace ze Servo/Firefoxu.

- [x] **A** — selectors crate — ✅ **doporučeno** · **zvoleno**
  - Ověřená správnost a výkon, MPL-2.0 závislost. Implementujeme jen rozhraní pro náš DOM.
- [ ] **B** — Vlastní selektorový engine
  - Kontrola a možnost optimalizací na míru, ale měsíce práce a snadné chyby ve specificitě.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q42 — Web components (Shadow DOM, custom elements) v 0.1?

*vyber jednu*

**Proč to řešíme:** Shadow DOM přináší izolaci stylů a scoped eventy – zásadní pro komponentové knihovny (Lit, Shoelace, FAST), ale komplikuje kaskádu, layout i eventy. Pozor: Speedometer 3.1 shadow DOM vyžaduje, takže pokud ho chceme v 0.1 jako bránu, tato otázka se mění na „musíme".

- [x] **A** — Odložit na 0.2, ale připravit DOM model (shadow root slot v NodeData) — ✅ **doporučeno** · **zvoleno**
  - Neušetříme si místo v datech, ale neučíme se složitost stylů a eventů dřív, než je nutné. Většina korpusu z Q02 ho nepotřebuje.
- [ ] **B** — Základ: open shadow root + custom elements v1 + slotted obsah
  - Otevře dveře komponentovým knihovnám. +4–8 týdnů a riziko chyb v kaskádě a eventech.
- [ ] **C** — Plná podpora včetně declarative shadow DOM a adoptedStyleSheets
  - Nejmodernější, ale výrazně dražší a pro 0.1 přehnané.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q43 — Iframes v 0.1?

*vyber jednu*

**Proč to řešíme:** Iframes jsou všude: vkládaná videa, mapy, platební brány, reklamy. Bez nich vypadá část top-100 webů prázdná. Plná izolace (OOPIF) je ale jiná liga.

- [ ] **A** — Základní same-process iframe se samostatným dokumentem — ✅ **doporučeno**
  - Vlastní DOM, styl a layout, sdílený proces; sandbox atribut částečně. Pokryje většinu vkládaného obsahu.
- [x] **B** — OOPIF (samostatný proces pro iframe) hned · **zvoleno**
  - Bezpečnostně správné, ale vyžaduje multi-proces (Q17) a je to měsíce práce.
- [ ] **C** — Žádné iframes v 0.1
  - Nejrychlejší, ale část korpusu bude vypadat rozbitě a videa/mapy nebudou fungovat.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q44 — Formulářové prvky v 0.1?

*vyber jednu*

**Proč to řešíme:** Formuláře jsou nutné pro Speedometer (inputy, tlačítka) i pro reálné weby. Speedometer navíc používá contenteditable editors, takže „jen input" nestačí. Potřebujeme textový editing model, focus, validaci a základní widgety (checkbox, radio, select, textarea).

- [x] **A** — Vlastní CSS-renderované ovládací prvky + textový editing model — ✅ **doporučeno** · **zvoleno**
  - input/textarea (včetně výběru, kurzoru, IME), button, checkbox, radio, select (základní), validace. Plná kontrola nad vzhledem a konzistencí s layoutem.
- [ ] **B** — Nativní macOS widgety (AppKit)
  - Automaticky správné chování a přístupnost, ale vzhled nepůjde stylovat CSS a vzhled bude „cizí".
- [ ] **C** — Minimum: jen input a button
  - Speedometer projde, reálné weby (select, checkbox, textarea) budou rozbité.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="9-layout-engine"></a>

## 9. Layout engine

Layout je skutečné know-how renderovacího jádra – a zároveň to, co se nedá koupit jako hotová knihovna. Flexbox a grid dnes umí taffy (produkčně používaný), ale blokový a inline layout s textem, floaty, tabulkami a scroll kontexty si musíme napsat sami.

Největší riziko není algoritmus, ale **inkrementalita**: pokud po každé změně přepočítáme celou stránku, Speedometer ani scroll na velkých webech nezvládneme.

### Q45 — Jak rozdělit práci na layoutu?

*vyber jednu*

**Proč to řešíme:** Psát flexbox a grid od nuly je měsíce a výsledek bude dlouho horší než taffy. Naopak blokový a inline layout s textem taffy neumí – tam je naše IP nevyhnutelná.

- [x] **A** — Hybrid: taffy pro flexbox a grid, vlastní blokový/inline/float/table layout — ✅ **doporučeno** · **zvoleno**
  - Nejrychlejší cesta ke správnému flexu a gridu. Taffy 0.15 umí flex, grid i základní blok a float, ale ne inline layout, text, ani tabulky – ty zůstávají na nás. Jádro layoutu (text, řádkování, tabulky, scroll) je naše IP.
- [ ] **B** — Vše vlastní včetně flexboxu a gridu
  - Plná kontrola a jednotný kód, ale realisticky +6 měsíců a vysoké riziko chyb v méně častých kombinacích.
- [ ] **C** — Taffy pro vše, co umí, zbytek obejít
  - Nejrychlejší, ale blokový a inline layout obejít nelze – text a dokumentový tok jsou jádro webu. Taffy navíc neumí tabulky ani inline obsah.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q46 — Tabulky v 0.1?

*vyber jednu*

**Proč to řešíme:** Tabulkový layout je historicky nejhorší část každého jádra (auto layout, colspan/rowspan, border-collapse). Na moderních webech se používá hlavně pro data, ne pro layout – ale dokumentace, e-shopy a e-maily ho používají pořád.

- [ ] **A** — Základní tabulkový layout (auto + fixed, colspan/rowspan, border-collapse) — ✅ **doporučeno**
  - Pokryje datové tabulky, které tvoří většinu reálného výskytu. Plná specifikace (separated borders, složité šířky) až později.
- [ ] **B** — Odložit na 0.2
  - Ušetří 3–5 týdnů, ale tabulky se rozpadnou na mnoha webech a v dokumentaci.
- [x] **C** — Plná implementace podle specifikace · **zvoleno**
  - Nejlepší výsledek, ale je to jeden z nejnáročnějších algoritmů v CSS a v 0.1 se nevyplatí.
- [ ] **jiné** — ______________________

**Zvoleno:** C · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q47 — Floaty a clear v 0.1?

*vyber jednu*

**Proč to řešíme:** Floaty jsou „legacy", ale pořád je používá mnoho webů a knihoven. Jejich implementace je středně náročná a ovlivňuje skoro celý blokový layout.

- [x] **A** — Základní floaty a clear (left/right, clear both, BFC) — ✅ **doporučeno** · **zvoleno**
  - Pokryje většinu reálného výskytu; složité kombinace s margin collapsing lze doladit později.
- [ ] **B** — Odložit
  - Ušetří čas, ale část starších webů a e-mailových šablon se rozsype.
- [ ] **C** — Plná specifikace včetně všech okrajových případů
  - Drahé a málo používané.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q48 — Inkrementální layout od začátku, nebo nejdřív plný přepočet?

*vyber jednu*

**Proč to řešíme:** Tohle rozhoduje o tom, zda Speedometer a scroll vůbec mohou být konkurenceschopné. Plný přepočet celé stránky při každé změně je jednoduchý, ale u velkých dokumentů neúnosný. Dodatečná inkrementalita znamená přepsat celý layout.

- [x] **A** — Inkrementální od začátku: dirty subtree + invalidační fronty — ✅ **doporučeno** · **zvoleno**
  - Změna stylu nebo DOMu označí jen dotčené podstromy; layout přepočítá potřebné minimum. Náročnější návrh, ale jediná cesta k výkonu.
- [ ] **B** — Nejdřív plný přepočet, inkrementalita v M4
  - Rychlejší první výsledky, ale hrozí, že se přepis nestihne a výkon zůstane špatný.
- [ ] **C** — Plný přepočet natrvalo
  - Nejjednodušší, ale s velkými dokumenty a animacemi nekonkurenceschopné.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q49 — Paralelizace layoutu?

*vyber jednu*

**Proč to řešíme:** Layout lze počítat paralelně po nezávislých podstromech, ale synchronizace a sdílené styly to komplikují. Apple Silicon má 4–6 P-core, takže potenciál je velký – ale až po funkčním základu.

- [ ] **A** — Layout běží na vlastním vlákně dokumentu; vnitřní paralelizace až po 0.1 — ✅ **doporučeno**
  - Oddělení od UI vlákna je nutné hned (jinak trhá scroll). Vnitřní dělení podstromů přidáme, až bude layout stabilní a bude co měřit.
- [x] **B** — Rayon a paralelní podstromy hned · **zvoleno**
  - Potenciálně velký zisk, ale v rané fázi přináší nedeterminismus a těžko se ladí.
- [ ] **C** — Vše na jednom vlákně
  - Nejjednodušší, ale scroll a animace budou trhat při každém layoutu.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q50 — Scroll kontexty, sticky a writing modes?

*vyber jednu*

**Proč to řešíme:** Moderní weby stojí na vnořených scroll kontejnerech (overflow: auto), sticky hlavičkách a fixed prvcích. Vertikální psaní a plné bidi jsou naopak vzácné a drahé.

- [x] **A** — Plné scroll kontejnery, sticky, fixed, scroll anchoring; jen horizontal-tb (LTR/RTL text přes shaping) — ✅ **doporučeno** · **zvoleno**
  - Pokryje naprostou většinu webů. RTL text se správně vykreslí, ale RTL layout stránky a vertikální psaní přijdou později.
- [ ] **B** — I vertikální writing modes a plný bidi layout
  - Pokryje japonštinu/arabštinu na úrovni layoutu, ale je to samostatný projekt a pro český/anglický korpus zbytečný.
- [ ] **C** — Jen scroll celého dokumentu, bez vnořených kontejnerů
  - Nejjednodušší, ale mnoho moderních aplikací (dashboardy, modaly) se rozbije.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="10-text-fonty-a-typografie"></a>

## 10. Text, fonty a typografie

Text je nejdůležitější část webu – a zároveň nejčastěji podceňovaná. Špatné tvarování (shaping), chybějící fallback font nebo ošklivý anti-aliasing zničí dojem z celého jádra, i kdyby layout byl dokonalý.

Dobrá zpráva: shaping i rasterizaci lze převzít z hotových Rust knihoven (rustybuzz/harfrust, swash) a soustředit se na glyfový atlas, cache a kvalitu výstupu. Špatná zpráva: kvalita textu je vizuálně nejvíc vidět, takže se v ní nedá šetřit.

### Q51 — Který shaping engine?

*vyber jednu*

**Proč to řešíme:** Shaping je převod textu na glyfy se správnými ligaturami, kerningem a pozicemi – pro latinku jednoduché, pro arabštinu, dévanágarí nebo emoji velmi složité. Pozor: `rustybuzz` je od roku 2026 archivovaný a jeho autoři sami doporučují přejít na `harfrust`.

- [x] **A** — harfrust — ✅ **doporučeno** · **zvoleno**
  - Rust port HarfBuzzu od lidí z HarfBuzz/Unicode, který sám HarfBuzz doporučuje jako nástupce rustybuzzu. Parita s HarfBuzz 15, méně než 25 % pomalejší než originál, aktivní vývoj (0.15, říjen 2026). Servo i cosmic-text už na něm staví.
- [ ] **B** — rustybuzz
  - Zralý a dobře známý, ale archivovaný a bez dalšího vývoje. Použitelné jen jako dočasný fallback.
- [ ] **C** — Vlastní shaping
  - Plná kontrola a žádná závislost, ale je to jeden z nejtěžších problémů v typografii; nedoporučuji.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q52 — Odkud brát fonty a fallback?

*vyber jednu*

**Proč to řešíme:** Bez správného fallbacku se česká diakritika, CJK nebo emoji zobrazí jako prázdné čtverečky. Zároveň každý zabudovaný font zvětšuje binárku.

- [x] **A** — Systémové fonty (fontdb) + malá zabudovaná sada fallbacků — ✅ **doporučeno** · **zvoleno**
  - macOS fonty pro běžný text; zabudované fallbacky pro emoji a základní CJK, aby weby vypadaly správně i bez instalace fontů. Rozumná velikost binárky.
- [ ] **B** — Jen systémové fonty
  - Nejmenší binárka, ale na webech s vlastními fonty a emoji bude vidět rozdíl proti Safari.
- [ ] **C** — Zabudovat velkou sadu (Noto komplet)
  - Nejlepší konzistence všude, ale stovky MB a pomalý start – proti našemu slibu nízké paměti.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q53 — Jak rasterizovat glyfy?

*vyber jednu*

**Proč to řešíme:** Rasterizace rozhoduje o ostrosti textu a výkonu. Cesty po glyfech jsou kvalitní, ale drahé; atlas s instancovanými quady je rychlý a škáluje.

- [x] **A** — swash rasterizace do glyfového atlasu, kreslení instancovanými quady přes vello — ✅ **doporučeno** · **zvoleno**
  - Rychlé, škálovatelné, kvalitní AA. Vello od 2026 nabízí `vello_gpu` se `Scene::glyph_run` (rasterizace glyfů na GPU + atlas), případně `glifo` + `vello_cpu` pro CPU cestu. Atlas se buduje postupně a drží v GPU paměti; ideální pro UMA.
- [ ] **B** — Každý glyf jako vektorová cesta
  - Nejlepší kvalita při libovolném zoomu, ale u dlouhých textů výrazně pomalejší.
- [ ] **C** — Bitové mapy předrenderované při startu
  - Predikovatelné, ale špatně se škáluje a neumí plynulý zoom ani velké fonty.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q54 — Jaký typografický cíl kvality?

*vyber jednu*

**Proč to řešíme:** „Krásné vykreslování" je subjektivní. Na Retina displejích se dnes používá hlavně grayscale AA s gamma korekcí; subpixel AA (ClearType styl) je na macOS/miniLED spíš na škodu. Rozhodnutí musí být explicitní, jinak se bude měnit podle nálady.

- [x] **A** — Retina-first: grayscale AA s gamma korekcí, jemné hinting, konzistence se Safari — ✅ **doporučeno** · **zvoleno**
  - Odpovídá tomu, jak dnes vypadají weby na Macu; nejlepší kompromis kvality a výkonu. Cílem je, aby text vypadal „stejně správně" jako v Safari.
- [ ] **B** — Subpixel AA pro non-Retina displeje
  - Ostřejší text na starších monitorech, ale na Retina/OLED může působit barevně a komplikuje kompozici.
- [ ] **C** — Čistě spec-neutral: bez gamma korekce a hintingu
  - Teoreticky „správné", ale vizuálně horší než konkurence – přesně to, čemu se chceme vyhnout.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q55 — Textový editing, kurzor, výběr a IME v 0.1?

*vyber jednu*

**Proč to řešíme:** Bez textového editingu nejde vyplnit formulář ani použít vyhledávání. Pro češtinu navíc potřebujeme mrtvé klávesy (ˇ, ´) a pro CJK IME. Toto je často přehlížená část, která rozhoduje o použitelnosti.

- [x] **A** — Plný základní editing: výběr, kurzor, mazání, schránka, mrtvé klávesy, IME — ✅ **doporučeno** · **zvoleno**
  - Nutné pro formuláře i Speedometer. Implementace je náročná (bidi výběr, IME kandidáti), ale bez ní jádro není použitelné.
- [ ] **B** — Jen jednoduchý input bez výběru a IME
  - Rychlé, ale čeština s diakritikou a CJK budou nefunkční; uživatelé to okamžitě poznají.
- [ ] **C** — Odloučit editing do 0.2
  - Ušetří týdny, ale Speedometer a formuláře nepůjdou – v rozporu s Q02/Q07.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q56 — Variabilní fonty, barevné fonty a emoji?

*vyber jednu*

**Proč to řešíme:** Moderní weby používají variabilní fonty (Inter, Recursive) a barevné emoji. Bez podpory vypadá typografie „z jiné éry".

- [x] **A** — Variabilní fonty + COLRv1 + systémové emoji — ✅ **doporučeno** · **zvoleno**
  - Pokryje moderní weby a emoji; COLRv1 je dnes standardní cesta pro barevné fonty.
- [ ] **B** — Jen variabilní fonty, emoji jako monochromatické
  - Jednodušší, ale emoji budou vypadat špatně – a těch je na webech hodně.
- [ ] **C** — Minimum: statické fonty, bez emoji podpory
  - Nejrychlejší, ale vizuálně nejvíc poznat.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="11-vykreslovani-kompozice-a-vizualni-kvalita"></a>

## 11. Vykreslování, kompozice a vizuální kvalita

Tady vzniká „fantastický" dojem – nebo taky trhání, duchové při scrollu a rozbité barvy. Moderní vykreslování není „nakresli pixely": je to strom vrstev, damage tracking, kompozice na GPU a správná práce s barvami.

Na Apple Silicon máme výhodu: Metal, UMA a 120 Hz ProMotion. Abychom ji využili, musí být kompozice navržená od začátku – ne jako vrstva přilepená na konec.

### Q57 — Jaký model vykreslování?

*vyber jednu*

**Proč to řešíme:** Okamžité překreslování celé stránky při každé změně je jednoduché, ale u velkých stránek a animací neúnosné. Retained vrstvy + display listy + damage umožňují překreslovat jen to, co se změnilo.

- [x] **A** — Retained strom vrstev + display listy + damage tracking — ✅ **doporučeno** · **zvoleno**
  - Změna se zapíše do display listu, poškozená oblast se překreslí, zbytek se znovu zkomponuje. Základ pro plynulý scroll a animace.
- [ ] **B** — Immediate mode: každý frame celá stránka
  - Nejjednodušší a bezstavové, ale s velkými dokumenty pomalé a energeticky náročné.
- [ ] **C** — Hybrid: retained jen pro animované vrstvy, zbytek immediate
  - Kompromis, ale pravidla „co je vrstva" jsou pak nepredikovatelná a vznikají vizuální chyby.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q58 — Kdy vytvářet samostatné kompoziční vrstvy?

*vyber jednu*

**Proč to řešíme:** Příliš mnoho vrstev žere paměť a GPU, příliš málo způsobuje trhání při scrollu a animacích. Heuristiky pro povýšení vrstvy jsou klíčové pro plynulost i spotřebu.

- [x] **A** — Automatické heuristiky: transform/opacity animace, fixed, video, canvas, scroll kontejnery, will-change — ✅ **doporučeno** · **zvoleno**
  - Stejný přístup jako moderní prohlížeče. Vrstvy vznikají jen tam, kde to pomůže; počet je omezený rozpočtem.
- [ ] **B** — Vrstvu jen na explicitní will-change
  - Predikovatelné a úsporné, ale weby bez will-change budou trhat.
- [ ] **C** — Jedna vrstva pro celou stránku
  - Nejjednodušší a nejúspornější, ale scroll velkých stránek bude pomalý.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q59 — Které kvalitativní vlastnosti jsou pro nás priorita?

*vyber vše, co platí*

**Proč to řešíme:** „Krásné vykreslování" je potřeba rozložit na měřitelné vlastnosti, jinak se nedá řídit ani obhájit. Tato volba určí, co budeme měřit a optimalizovat.

- [x] **A** — Kvalitní anti-aliasing hran a textu — ✅ **doporučeno** · **zvoleno**
  - Nejvíc viditelný rozdíl proti „rychlému" vykreslování.
- [x] **B** — Správná správa barev (sRGB + Display P3) — ✅ **doporučeno** · **zvoleno**
  - Na Macu s P3 displejem je rozdíl vidět; konkurence to řeší nekonzistentně.
- [x] **C** — HDR obsah · **zvoleno**
  - Efektní, ale v 0.1 nemáme video ani HDR obrázky – odložit.
- [x] **D** — Ostrý text na Retina — ✅ **doporučeno** · **zvoleno**
  - Základ čitelnosti; úzce souvisí s Q54.
- [x] **E** — Kvalitní stíny, blur a gradienty bez pruhů — ✅ **doporučeno** · **zvoleno**
  - Nejvíc „prémiový" pocit z UI; dithering gradientů je levný a hodně viditelný.
- [x] **F** — Plynulost 120 Hz a frame pacing — ✅ **doporučeno** · **zvoleno**
  - ProMotion je hlavní výhoda Macu; bez správného pacing vypadá scroll hůř než 60 Hz.
- [x] **G** — backdrop-filter (sklo) · **zvoleno**
  - Moderní efekt, ale drahý; v 0.1 volitelně.
- [x] **H** — Věrnost vůči referenčnímu renderu (deltaE/SSIM) — ✅ **doporučeno** · **zvoleno**
  - Objektivní měření kvality proti Safari/Chromu.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F, G, H · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q60 — Které vizuální efekty musí 0.1 umět?

*vyber vše, co platí*

**Proč to řešíme:** Efekty jsou to, co dělá moderní UI moderním – ale každý je jinak drahý. Rozhodnutí určí, jak bude vypadat náš korpus a kolik času strávíme v shaderech.

- [x] **A** — border-radius a box-shadow — ✅ **doporučeno** · **zvoleno**
  - Naprostý základ moderního UI.
- [x] **B** — Gradienty (linear, radial, conic) s ditheringem — ✅ **doporučeno** · **zvoleno**
  - Všude; bez ditheringu vznikají pruhy.
- [x] **C** — Filtry: blur, brightness, contrast, saturate — ✅ **doporučeno** · **zvoleno**
  - Běžné v UI a obrázcích; blur je nejdražší.
- [x] **D** — clip-path a masky — ✅ **doporučeno** · **zvoleno**
  - Čím dál běžnější (karty, ilustrace, přechody).
- [x] **E** — Mix-blend-mode · **zvoleno**
  - Efektní, ale méně časté; lze odložit.
- [ ] **F** — backdrop-filter
  - Glassmorphism; drahé, ale velmi viditelné. V 0.1 jako stretch.
- [x] **G** — CSS transitions a keyframe animace — ✅ **doporučeno** · **zvoleno**
  - Nutné pro pocit živého UI.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, G · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q61 — Canvas, obrázky, SVG a video v 0.1?

*vyber jednu*

**Proč to řešíme:** Canvas používá Speedometer (grafy) i mnoho webů. SVG je všude (ikony, ilustrace). Video je energeticky nejnáročnější scénář a zároveň nejlepší argument pro náš „energetický" slib – ale vyžaduje AVFoundation/VideoToolbox.

- [ ] **A** — Obrázky + SVG + Canvas 2D; video až v 0.2 — ✅ **doporučeno**
  - Pokryje grafiku a benchmarky. Video odložíme, ale navrhneme místo pro video vrstvu (kompozice, overlay controls).
- [ ] **B** — A + WebGL/WebGPU
  - Otevře 3D weby a hry, ale je to samostatný velký projekt (GLSL/WGSL kompatibilita, validace).
- [ ] **C** — A + video přes AVFoundation
  - Umožní měřit spotřebu energie při přehrávání – náš nejsilnější příběh. Ale +4–6 týdnů a další platformní vazba.
- [x] **D** — Jen obrázky, bez canvasu a SVG · **zvoleno**
  - Nejrychlejší, ale Speedometer neprojde a weby s ikonami budou rozbité.
- [ ] **jiné** — ______________________

**Zvoleno:** D · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

---

<a id="12-javascript-webassembly-a-bindingy"></a>

## 12. JavaScript, WebAssembly a bindingy

Tohle je největší „koupit vs. postavit" rozhodnutí. JavaScript engine je uzavřený systém s jasnou specifikací a test262 – na rozdíl od HTML parseru se dá teoreticky napsat. Ale výkon moderního JS je výsledek desítek let práce na JIT, inline caching a GC. Boa je správná, ale řádově pomalejší; vlastní engine srovnatelný s V8 je projekt na roky.

Pro srovnání s Chrome na Speedometeru je výhoda mít **stejný JS engine**: rozdíl ve skóre pak měří přesně to, co jsme napsali my (layout, styl, paint). To je vědecky čistší a marketingově silnější.

### Q62 — Který JavaScript engine?

*vyber jednu*

**Proč to řešíme:** Volba určuje výkon, velikost binárky, složitost buildování a hlavně to, jak bezpečně půjde propojit JS s naším DOM. Je to **jednosměrné rozhodnutí** – musí padnout předtím, než zamrazíme objektový model DOM. Z výzkumu: `mozjs` je jediné bindingy s produkčním prohlížečem za sebou a s přesným trasováním Rust objektů; `deno_core` je výkonnější ve JS, ale je stavěné jako runtime, ne jako DOM engine.

- [ ] **A** — V8 (crate `v8`, případně `deno_core`)
  - Nejvyšší JS výkon a stejný základ jako Chrome, takže benchmarky měří naši práci. Ale: `deno_core` je runtime-shaped, nemá přesné host-GC trasování (jen Weak/TracedReference), každé čtvrtletí breaking API a ~100 MB+. Použitelné, pokud jsme ochotni si cppgc vrstvu napsat sami.
- [x] **B** — SpiderMonkey (crate `mozjs`) — ✅ **doporučeno** · **zvoleno**
  - Engine Firefoxu, Servo ho používá v produkci. Klíčové: TraceTrait přesně trasuje Rust-owned DOM uzly, takže GC cykly DOM↔JS jdou řešit správně – přesně tam, kde Ladybird unikal 15 GB. Předkompilované archivy pro arm64, MPL-2.0. Slabší JS výkon než V8 a méně pohodlné API, ale pro nás je to ta správná strana kompromisu.
- [ ] **C** — JavaScriptCore (JSC)
  - Engine Safari, na macOS nejlépe integrovaný a energeticky nejúspornější. Ale embedderské API je méně vhodné pro vlastní DOM a ladění je obtížnější.
- [ ] **D** — Boa (čistě Rust)
  - Žádná C++ závislost, snadná integrace, ale výkon je řádově jinde – Speedometer by nebyl konkurenceschopný. Vhodné jen pro experimenty.
- [ ] **E** — Vlastní engine od nuly
  - Největší „from scratch" příběh, ale je to projekt na roky a výsledek bude dlouho horší než Boa. Nedoporučuji pro 0.1.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q63 — Jak generovat DOM bindingy pro JS?

*vyber jednu*

**Proč to řešíme:** Web IDL má stovky rozhraní. Ruční psaní bindingů je pomalé, náchylné k chybám a nekonzistentní. Generátor z IDL je standardní řešení (Chromium, Gecko) a jediné rozumné pro rozsah, který chceme.

- [x] **A** — Vlastní generátor z Web IDL (build.rs) + IDL soubory ze specifikace — ✅ **doporučeno** · **zvoleno**
  - Konzistentní, rychlé a rozšiřitelné; jednou napsaný generátor ušetří měsíce. Vyžaduje ale inicialní investici a testy (idlharness).
- [ ] **B** — Ručně psané bindingy
  - Nejlepší kontrola a výkon pro horké cesty, ale u stovek rozhraní neudržitelné.
- [ ] **C** — Hybrid: generátor + ruční optimalizace horkých cest
  - Pragmatické a výkonné, ale složitější údržba. Vhodné jako evoluce varianty A.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q64 — Jak řešit vztah JS GC a DOM?

*vyber jednu*

**Proč to řešíme:** DOM uzly drží JS objekty a JS objekty drží DOM uzly – vznikají cykly. Špatné řešení znamená unikající paměť nebo pády při uvolnění. V8 nabízí traced wrappers (cppgc), které tomuto přesně čelí.

- [x] **A** — Přesné trasování (mozjs TraceTrait, případně V8 cppgc/Traceable) — ✅ **doporučeno** · **zvoleno**
  - Správné a bezpečné řešení: GC vidí naše DOM uzly a může je uvolnit i v cyklech. `mozjs` to má hotové přes TraceTrait, V8 to řeší cppgc. Vyžaduje pečlivé trace funkce, ale řeší cykly i přírůstkové uvolňování.
- [ ] **B** — Weak reference + ruční úklid
  - Jednodušší, ale snadno vzniknou cykly, které nikdy nezmizí – přesně to, kvůli čemu jsou prohlížeče paměťově náročné.
- [ ] **C** — Vlastní GC pro DOM
  - Maximální kontrola, ale psát GC je samostatný výzkum; nedoporučuji.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q65 — WebAssembly v 0.1?

*vyber jednu*

**Proč to řešíme:** Wasm potřebují moderní aplikace (Figma, Google Workspace, herní enginy) a je to pákový efekt na výkon webu. Zároveň je to další runtime, další testovací matice a potenciální bezpečnostní plocha.

- [ ] **A** — Ne v 0.1, ale s připraveným rozhraním (feature flag) — ✅ **doporučeno**
  - Soustředíme se na renderovací jádro. Wasmtime přidáme v 0.2 – architektura s ním počítá.
- [x] **B** — Ano, wasmtime hned · **zvoleno**
  - Otevře moderní aplikace a je to skvělý marketing, ale +4–8 týdnů a další závislost.
- [ ] **C** — Vlastní Wasm interpret/JIT
  - Zajímavý výzkum, ale v 0.1 nemá místo.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q66 — Moduly, workers a event loop JS?

*vyber jednu*

**Proč to řešíme:** Moderní weby používají ES moduly a import maps; část z nich i Web Workers. Worker znamená druhý JS kontext a sdílení/klonování dat – další složitost, ale bez něj některé aplikace zamrznou.

- [x] **A** — ES moduly + import maps; workery až v 0.2 — ✅ **doporučeno** · **zvoleno**
  - Pokryje většinu moderních webů. Workery navrhneme v API, ale implementace počká.
- [ ] **B** — I dedikované workery v 0.1
  - Víc reálných aplikací projde, ale strukturální clone, přenos ArrayBufferů a ladění kontextů jsou drahé.
- [ ] **C** — Jen klasické skripty bez modulů
  - Nejrychlejší, ale moderní weby (Vite, Next.js) nepojedou.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="13-web-api-surface-a-formulare"></a>

## 13. Web API surface a formuláře

Jádro není jen vykreslování – weby ožívají díky Web API. Zároveň platí, že čím víc API, tím víc práce a tím víc bezpečnostních děr. Pro 0.1 potřebujeme přesně tu množinu, která pokryje korpus a Speedometer, a nic navíc „protože by to bylo hezké".

Pravidlo: každé API musí mít konkrétní web nebo test, který ho vyžaduje. Jinak jde do backlogu.

### Q67 — Která Web API musí 0.1 mít?

*vyber vše, co platí*

**Proč to řešíme:** Tohle je páteř interaktivity. Bez API níže nefunguje žádný moderní web ani benchmark. Naopak API jako Service Workers nebo IndexedDB jsou drahá a pro korpus z Q02 nepotřebná.

- [x] **A** — DOM, CSSOM, EventTarget, CustomEvent — ✅ **doporučeno** · **zvoleno**
  - Základ všeho; bez toho nejde ani jeden framework.
- [x] **B** — Fetch + XMLHttpRequest — ✅ **doporučeno** · **zvoleno**
  - Načítání dat; XHR je starý, ale stále používaný knihovnami.
- [x] **C** — History + Location (pushState, hashchange, navigace) — ✅ **doporučeno** · **zvoleno**
  - SPA routování; bez toho se React Router a spol. rozbijí.
- [x] **D** — localStorage + sessionStorage — ✅ **doporučeno** · **zvoleno**
  - Přihlášení, témata, košík; nejpoužívanější storage API.
- [x] **E** — Timers, requestAnimationFrame, requestIdleCallback — ✅ **doporučeno** · **zvoleno**
  - Animace a plánování; rAF je kritické pro plynulost.
- [x] **F** — IntersectionObserver, ResizeObserver, MutationObserver — ✅ **doporučeno** · **zvoleno**
  - Lazy-loading obrázků, responzivní komponenty; dnes standard.
- [x] **G** — URL, URLSearchParams, Blob, File, FormData, atob/btoa — ✅ **doporučeno** · **zvoleno**
  - Práce s daty a formuláři; levné a široce používané.
- [x] **H** — Layout API: getBoundingClientRect, getComputedStyle, scrollTo, scrollIntoView — ✅ **doporučeno** · **zvoleno**
  - Každá knihovna ho používá pro pozicování tooltipů a animací.
- [x] **I** — Focus, selection, clipboard (základ) — ✅ **doporučeno** · **zvoleno**
  - Přístupnost a kopírování; clipboard vyžaduje permissions.
- [x] **J** — WebSocket + EventSource — ✅ **doporučeno** · **zvoleno**
  - Realtime aplikace; WebSocket je levný, pokud už máme síť.
- [x] **K** — crypto.getRandomValues, matchMedia, Performance API — ✅ **doporučeno** · **zvoleno**
  - Bezpečnostní primitiva a responzivita; matchMedia je nutné pro media queries v JS.
- [x] **L** — Canvas 2D API — ✅ **doporučeno** · **zvoleno**
  - Grafy a vizualizace; Speedometer ho používá.
- [x] **M** — IndexedDB, Service Workers, WebRTC, Geolocation, Notifications · **zvoleno**
  - Drahé a pro 0.1 nepotřebné; odložit.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F, G, H, I, J, K, L, M · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q68 — Storage nad rámec localStorage?

*vyber jednu*

**Proč to řešíme:** IndexedDB je výkonné a používá ho mnoho aplikací, ale vyžaduje asynchronní API, transakce a verzování schématu. Pro 0.1 je to velký kus práce s malým dopadem na korpus.

- [x] **A** — Jen localStorage a sessionStorage; IndexedDB v 0.2 — ✅ **doporučeno** · **zvoleno**
  - Pokryje přihlášení a nastavení; weby, které potřebují IDB, budou mít omezenou funkčnost – zdokumentujeme.
- [ ] **B** — I základní IndexedDB
  - Víc aplikací projde, ale +4–6 týdnů a složité testování.
- [ ] **C** — Žádný storage
  - Nejrychlejší, ale přihlášení a personalizace nefungují.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q69 — Integrace s platformou (soubory, schránka, oprávnění)?

*vyber jednu*

**Proč to řešíme:** Nahrávání souborů a schránka jsou nutné pro reálné použití; notifikace a geolokace vyžadují UI pro oprávnění a jsou na začátek zbytečné. Zároveň jde o bezpečnostně citlivé API.

- [ ] **A** — File input, stahování, schránka, jednoduchý permissions model — ✅ **doporučeno**
  - Pokryje reálné weby a je to bezpečně zvládnutelné. Notifikace, geolokace a USB/NFC až později.
- [x] **B** — Všechno včetně notifikací a geolokace · **zvoleno**
  - Víc kompletní, ale vyžaduje UI oprávnění a řešení soukromí – odvádí od jádra.
- [ ] **C** — Nic z toho
  - Nejjednodušší, ale nahrávání souborů je základní funkce webu.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q70 — Přístupnost (accessibility) v 0.1?

*vyber jednu*

**Proč to řešíme:** Přístupnost je správná věc a zároveň základ pro AI vrstvu (sémantický strom stránky). Plná integrace s VoiceOver je ale samostatný projekt. Rozumný kompromis: správný AX strom v jádře, systémová integrace později.

- [ ] **A** — Strom AX rolí a jmen v jádře + export pro embedder a AI; VoiceOver most později — ✅ **doporučeno**
  - Řeší obojí: přístupnost i AI. Most do macOS Accessibility API přidáme, až bude strom stabilní.
- [x] **B** — Plná integrace s VoiceOverem hned · **zvoleno**
  - Nejlepší pro uživatele, ale je to měsíce práce a silná vazba na AppKit.
- [ ] **C** — Žádná accessibility
  - Nejrychlejší, ale veřejný projekt si to nemůže dovolit – a přišli bychom o základ pro AI.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

---

<a id="14-mereni-jak-prokazeme-ze-jsme-dobri"></a>

## 14. Měření: jak prokážeme, že jsme dobří

Bez měření je „překonat nejlepší" jen slogan. Zároveň platí zásadní věc z výzkumu: **Speedometer 3.1 a vykreslování top webů jsou dva různé cíle.** Speedometer neobsahuje tabulky, floaty, SVG styling, bidi, formuláře ani média – měří hlavně JS, DOM operace a event loop. Ladybirdův skok o 50 % na Speedometeru vznikl optimalizací timer clampingu a potlačením rAF v offscreen dokumentech, ne layoutem.

Druhá tvrdá věc: **Speedometer vyžaduje skoro celý prohlížeč** – reálné okno ≥850×650, iframy, shadow DOM, contenteditable, canvas 2D a SVG. Rozhodnutí, zda ho chceme v 0.1, je tedy rozhodnutím o rozsahu, ne o benchmarku.

A třetí: **JetStream 3.0** (vydaný 2026-03-31) je jediný velký benchmark, který běží v shellu bez DOM – ideální první automatizovaný signál. Čísla z JetStream 2 se s JetStream 3 nedají srovnávat.

### Q71 — Kdy se Speedometer stane součástí brány?

*vyber jednu*

**Proč to řešíme:** Speedometer 3.1 není jen číslo – jeho harness vyžaduje reálné okno ≥850×650, iframy, shadow DOM, contenteditable, canvas 2D a SVG. To je skoro celý prohlížeč. Pokud ho chceme v 0.1 jako blokující bránu, musíme do 0.1 dostat shadow DOM i contenteditable, což je +3–4 měsíce práce. Ladybird měl po 7 letech a 1,19 mil. řádků ~5,9; Safari 26.2 na M2 dává 41,9.

- [x] **A** — Speedometer až od 0.2; v 0.1 měříme JetStream 3 (běží v shellu) + vlastní responsiveness harness na korpusu — ✅ **doporučeno** · **zvoleno**
  - JetStream 3 je jediný velký benchmark, který běží bez DOM – ideální první automatizovaný signál. Vlastní harness na korpusu měří to, co skutečně umíme. Doporučeno.
- [ ] **B** — Speedometer v 0.1, a tedy shadow DOM + contenteditable + iframe + canvas + SVG už v 0.1
  - Nejlepší PR číslo, ale posouvá 0.1 o 3–4 měsíce a rozporuje řezy z Q42 a Q43. Cíl by byl ≥25 (≈60 % Safari).
- [ ] **C** — Speedometer jen orientačně, nikdy jako blokující brána
  - Měříme ho, ale neslibujeme číslo. Nejbezpečnější, ale méně motivující.

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q72 — Jaký konkrétní WPT cíl pro 0.1?

*vyber jednu*

**Proč to řešíme:** WPT je jediné objektivní měřítko správnosti a dnes je to i regulatorní laťka: Apple v EU vyžaduje pro alternativní engine 90 % WPT a 80 % test262 (a to i s vypnutým JIT). Aktuální stabilní prohlížeče: Chrome 97,1 %, Firefox 95,6 %, Safari 94,2 %, Servo 90,4 % (aligned 88,3). Cíl musí být ambiciózní, ale ne nesplnitelný.

- [x] **A** — 55 % celkově, 80 % /dom/, 75 % /html/, 40 % /css/ (stretch dle matice) · **zvoleno**
  - Nejambicióznější realistická varianta pro 12 měsíců; vyžaduje ale široké API pokrytí.
- [ ] **B** — 40 % celkově, 65 % /dom/, 60 % /html/, 30 % /css/ – veřejně publikované — ✅ **doporučeno**
  - Dosažitelné s rozsahem z Q07 varianty B a zároveň číslo, které lze obhájit. Doporučeno.
- [ ] **C** — Žádný absolutní cíl, jen meziroční růst
  - Bezpečné, ale „projekt žije" potřebuje konkrétní číslo, ne jen křivku.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q73 — Které benchmarky a metriky budou součástí bran (gates)?

*vyber vše, co platí*

**Proč to řešíme:** Každý benchmark v bráně znamená, že regrese blokuje merge. Příliš mnoho = pomalé CI a falešné poplachy; příliš málo = regrese se neodhalí. Toto je páteř našeho „měřitelného" slibu.

- [x] **A** — Speedometer 3.1 — tracking od 0.2 (v 0.1 jen orientačně) · **zvoleno**
  - Vyžaduje shadow DOM, contenteditable, iframy, canvas a SVG. Do brány ho zařadíme, až bude celý stack hotový (viz Q71).
- [x] **B** — MotionMark — ✅ **doporučeno** · **zvoleno**
  - Grafický výkon a plynulost animací – nejlépe zachycuje naši GPU výhodu.
- [x] **C** — Vlastní korpusová sada (screenshoty + vizuální diff) — ✅ **doporučeno** · **zvoleno**
  - Nejdůležitější pro skutečnou kvalitu: top-100 a dokumentační weby proti referenčnímu renderu.
- [x] **D** — WPT subset (DOM, CSS, HTML, events) — ✅ **doporučeno** · **zvoleno**
  - Objektivní míra správnosti specifikace; veřejné číslo, které jde sledovat v čase.
- [x] **E** — Paměť: footprint na kartu při N otevřených dokumentech — ✅ **doporučeno** · **zvoleno**
  - Náš hlavní slib. Měříme stejný korpus jako konkurence, stejnou metodou.
- [x] **F** — Energie: powermetrics při scrollu a animaci — ✅ **doporučeno** · **zvoleno**
  - Druhý hlavní slib; měření je náročné na stabilitu, ale stojí za to.
- [x] **G** — Studený start a first contentful paint — ✅ **doporučeno** · **zvoleno**
  - Snadno měřitelné a uživatelsky důležité.
- [ ] **H** — JetStream 3.0 (JS/Wasm, běží v shellu) — ✅ **doporučeno**
  - Jediný velký benchmark, který nepotřebuje DOM – první automatizovaný signál a kontrola, že jsme JS engine nerozbili.
- [x] **I** — Velikost binárky a doba buildu · **zvoleno**
  - Užitečné pro vývojářský zážitek, ne pro uživatele.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F, G, I · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q74 — Proti čemu a jak budeme měřit?

*vyber jednu*

**Proč to řešíme:** Srovnávat s čísly z internetu je nepoctivé – jiný hardware, jiná verze, jiná nastavení. Jediné obhajitelné srovnání je na stejném stroji, se stejnými weby a stejnou metodikou.

- [x] **A** — Stejný M4, stejný korpus, Safari + Chrome + my; mediány, warmup, zamčené verze, publikovaná metodika — ✅ **doporučeno** · **zvoleno**
  - Jediné srovnání, které obstojí. Naměřená data a skripty zveřejníme, aby si to mohl zopakovat kdokoli.
- [ ] **B** — Srovnání s publikovanými čísly výrobců
  - Nejjednodušší, ale neobhajitelné – vždy se najde někdo, kdo zpochybní hardware i nastavení.
- [ ] **C** — Jen interní regresní trackování, bez veřejného srovnání
  - Bezpečné, ale přicházíme o hlavní marketingový i motivační efekt.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q75 — Jak budeme publikovat výsledky?

*vyber jednu*

**Proč to řešíme:** Veřejný dashboard je nejsilnější důkaz „projekt žije" a zároveň nejlepší obrana proti nařčení z cherry-pickingu. Zároveň je to závazek, který bolí, když se něco zhorší.

- [x] **A** — Veřejný dashboard s historií, všechny běhy, včetně regresí — ✅ **doporučeno** · **zvoleno**
  - Maximální důvěryhodnost a skvělý signál aktivity. Vyžaduje stabilní infrastrukturu a disciplínu.
- [ ] **B** — Výsledky v devlogu při milnících
  - Méně práce, stále užitečné, ale bez kontinuity a snadno se vynechá.
- [ ] **C** — Nepublikovat nic do 0.1
  - Nejmenší tlak, ale ztrácíme příběh a možnost ukázat pokrok.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q76 — Kde budeme hledat výkonnostní výhru?

*vyber vše, co platí*

**Proč to řešíme:** Ne všechny optimalizace mají stejný efekt. Z výzkumu je jasné, že největší rozdíl dělá invalidace a hygiena event loopu, ne surový výpočet. Tato volba určí, kam půjde čas v M4.

- [x] **A** — Inkrementální styl a layout (dirty roots, minimal invalidation) — ✅ **doporučeno** · **zvoleno**
  - Největší páka na reálných webech i Speedometeru. Ladybird i Servo tady roky ladí okrajové případy.
- [x] **B** — Hygiena event loopu (timer clamping, skryté dokumenty, rAF) — ✅ **doporučeno** · **zvoleno**
  - Překvapivě velký vliv na Speedometer; levné a měřitelné.
- [x] **C** — GPU kompozice a damage tracking — ✅ **doporučeno** · **zvoleno**
  - Plynulost scrollu a animací, energetická výhoda.
- [x] **D** — Paměťová optimalizace a cache — ✅ **doporučeno** · **zvoleno**
  - Náš hlavní slib; často jde o návrh datových struktur, ne o mikrooptimalizace.
- [ ] **E** — JS/JIT optimalizace
  - S V8 nemáme co zlepšovat – a to je dobře, protože benchmark pak měří naši práci.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="15-bezpecnost-sandbox-a-soukromi"></a>

## 15. Bezpečnost, sandbox a soukromí

Prohlížečové jádro zpracovává nedůvěryhodný obsah z internetu – je to nejzranitelnější typ software, jaký běžně používáme. Rust nám odstraní celou třídu chyb (buffer overflow, use-after-free), ale ne všechny: zůstávají logické chyby, chyby v sandboxu, úniky dat mezi originy a útoky přes JIT.

V 0.1 nemusíme mít site isolation, ale musíme mít **správný origin model a čisté hranice**, protože ty se dodatečně zavádějí nejhůř.

### Q77 — Origin model, same-origin policy a CORS v 0.1?

*vyber jednu*

**Proč to řešíme:** Bez SOP a CORS nejde bezpečně otevřít dva weby v jednom procesu – jeden by mohl číst data druhého. Zároveň je to nutné pro funkčnost moderních API (fetch, cookies, storage).

- [x] **A** — Plný origin model + SOP + CORS (preflight, credentials, opaque origins) — ✅ **doporučeno** · **zvoleno**
  - Nutné pro jakýkoli reálný web. Implementace je dobře specifikovaná a testovatelná (WPT).
- [ ] **B** — Jen SOP bez CORS
  - Rychlejší, ale moderní weby komunikující s API na jiné doméně nebudou fungovat.
- [ ] **C** — Žádný origin model (dev režim)
  - Pouze pro prototypy; v 0.1 nepoužitelné a nebezpečné.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q78 — Jaký sandbox a hardening v 0.1?

*vyber jednu*

**Proč to řešíme:** macOS nabízí hardened runtime, App Sandbox a seatbelt profily. Plná izolace procesů je až s multi-procesem (Q17), ale hardening kompilace a entitlements jdou udělat hned a jsou levné.

- [x] **A** — Hardened runtime + JIT entitlement + připravené seatbelt profily pro budoucí procesy — ✅ **doporučeno** · **zvoleno**
  - Reálná úroveň ochrany bez multi-procesu; příprava na 0.2, kdy půjde síť/GPU do vlastních procesů.
- [ ] **B** — Plný App Sandbox od začátku
  - Nejbezpečnější a nutné pro Mac App Store, ale omezuje přístup k souborům a komplikuje vývoj; pro vývojářský nástroj v 0.1 zbytečné.
- [ ] **C** — Bez hardeningu, jen Rust safety
  - Nejrychlejší, ale u jádra, které zpracovává cizí obsah, to není obhajitelné.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q79 — Content Security Policy v 0.1?

*vyber jednu*

**Proč to řešíme:** CSP je hlavní obrana proti XSS a dnes ho používá většina velkých webů. Plná úroveň 3 je složitá; základní úroveň 2 pokryje většinu reálných hlaviček.

- [ ] **A** — CSP Level 2 subset + report-only mód + nonce/hash — ✅ **doporučeno**
  - Pokryje reálné weby a je testovatelné. Level 3 (trusted types, strict-dynamic detaily) později.
- [x] **B** — Plná CSP Level 3 · **zvoleno**
  - Nejlepší ochrana, ale výrazně víc práce a okrajových případů.
- [ ] **C** — Žádná CSP
  - Jednodušší, ale weby s CSP se mohou chovat jinak a bezpečnostní příběh bude slabý.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q80 — Které bezpečnostní praktiky budou povinné od prvního commitu?

*vyber vše, co platí*

**Proč to řešíme:** Dodatečné zavádění fuzzingu a supply-chain kontrol je drahé a často se odkládá navždy. Přitom jde o hodiny až dny práce, které zachytí chyby dřív, než se dostanou do mainu.

- [x] **A** — cargo-deny (licence, duplicity, zranitelnosti) v CI — ✅ **doporučeno** · **zvoleno**
  - Levné a okamžitě užitečné.
- [ ] **B** — cargo-vet / audit závislostí
  - Pro prohlížečové jádro s desítkami závislostí rozumné; lze zavést postupně.
- [x] **C** — Miri pro všechny unsafe bloky — ✅ **doporučeno** · **zvoleno**
  - Odhalí undefined behavior v našem unsafe kódu (GPU/FFI); pomalé, ale běží jen na části testů.
- [x] **D** — cargo-fuzz na parser a layout (24h běh v CI/nightly) — ✅ **doporučeno** · **zvoleno**
  - Nejlepší investice do robustness parsingu a layoutu.
- [ ] **E** — AddressSanitizer/UBSan build v CI
  - Užitečné hlavně pro FFI a C++ závislosti (V8); náročné na čas CI.
- [x] **F** — Povinné review u změn v bezpečnostně citlivých crates · **zvoleno**
  - Procesní opatření; v agentním vývoji obzvlášť důležité.
- [ ] **jiné** — ______________________

**Zvoleno:** A, C, D, F · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

---

<a id="16-ai-a-agentni-vrstva"></a>

## 16. AI a agentní vrstva

Z výzkumu i z praxe (Chrome/Gemini, Edge/Copilot, Huawei Web Agent) plyne jasné doporučení: **AI inference nepatří do jádra.** Jádro má zůstat „hloupé a extrémně rychlé" – a místo modelů nabídnout strukturovaný, sémantický popis stránky a bezpečné akční API. Veškerá inteligence, modely a orchestrace patří do vrstvy prohlížeče nebo do samostatného procesu.

To ale neznamená, že AI ignorujeme. Znamená to, že ji **navrhneme jako rozhraní hned**, protože dodatečné „vykuchání" sémantiky z hotového jádra je drahé a nepřesné.

### Q81 — Kde je hranice mezi jádrem a AI?

*vyber jednu*

**Proč to řešíme:** Pokud AI runtime vstoupí do jádra, přinese obrovské nároky na paměť, nestabilní latenci a bezpečnostní rizika (prompt injection) přímo do cesty kritické pro vykreslování. Pokud naopak jádro neumí vydat sémantiku, AI vrstva bude „koukat na screenshoty" – drahé a nespolehlivé.

- [x] **A** — Jádro exportuje sémantický strom + akční API; AI runtime žije výš — ✅ **doporučeno** · **zvoleno**
  - Jádro dodá role, jména, souřadnice, stav interaktivity a stabilní ID; AI vrstva (nebo samostatný proces) nad ním plánuje a volá akce. Čistá hranice, testovatelné, bezpečné.
- [ ] **B** — AI inference přímo v jádře
  - Nejjednodušší integrace pro uživatele, ale modely žerou RAM, blokují práci s pamětí a přinášejí bezpečnostní rizika. Nedoporučuji.
- [ ] **C** — Samostatný AI sidecar proces s protokolem (MCP-like)
  - Nejbezpečnější a nejflexibilnější, ale vyžaduje IPC a složitější distribuci. Dobrá varianta pro 0.2+.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q82 — V jakém formátu má jádro vydávat stránku pro AI?

*vyber jednu*

**Proč to řešíme:** Tohle je náš „Annotated Page Content". Pokud ho navrhneme dobře, ušetří AI vrstvě tisíce tokenů a umožní spolehlivé akce. Pokud špatně, nikdo ho nepoužije.

- [x] **A** — JSON schema: uzly s ID, rolí, jménem, bounding boxem, stavem, textem + seznam dostupných akcí — ✅ **doporučeno** · **zvoleno**
  - Kompaktní, stabilní ID přežívají mezi snímky, souřadnice umožňují fallback na kliknutí myší. Doporučeno.
- [ ] **B** — Jen accessibility tree
  - Standardizované a levné, ale chybí geometrie a vizuální kontext; AI hůř chápe layout.
- [ ] **C** — HTML snapshot / DOM dump
  - Nejjednodušší, ale obrovské tokeny, nestabilní a plné šumu. Přesně to, čemu se moderní AI prohlížeče vyhýbají.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q83 — Kolik AI chceme v 0.1?

*vyber jednu*

**Proč to řešíme:** AI je lákavá, ale odvádí od jádra. Rozumné je dodat rozhraní a nechat integraci na 0.2 – pokud ale chceš AI demo kvůli publicitě, musíme s ním počítat v milnících.

- [x] **A** — Jen návrh a export API (sémantický strom, akce), žádné modely — ✅ **doporučeno** · **zvoleno**
  - Jádro zůstává čisté, AI vrstva může vzniknout kdykoli potom. Doporučeno.
- [ ] **B** — A + demo agent nad jádrem (mimo repo jádra)
  - Skvělá ukázka a marketing, ale je to samostatný projekt; musíme ho od jádra oddělit, aby nezdržoval.
- [ ] **C** — Plná lokální AI integrace (Core ML / MLX)
  - Velmi atraktivní na Apple Silicon, ale je to jiný projekt s jinými riziky (paměť, modely, soukromí).
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="17-dokumentace-ci-a-release"></a>

## 17. Dokumentace, CI a release

Chceš, aby byl projekt použitelný jinými vývojáři a aby bylo vidět, že žije. To není o marketingu, ale o nudné disciplíně: každý veřejný symbol má dokumentaci, každý modul má popis a invarianty, každý commit má smysl, každý release má changelog a každé číslo má metodiku.

Dokumentace je zároveň nejlepší forma testu – pokud nedokážeme popsat, co modul dělá, pravděpodobně tomu nerozumíme.

### Q84 — Jaký rozsah češtiny v dokumentaci?

*vyber jednu*

**Proč to řešíme:** Plná dvojjazyčná dokumentace včetně API referencí je překladatelský projekt sám o sobě a při každé změně kódu se rozjíždí. Zahraniční vývojáři potřebují angličtinu; ty potřebuješ češtinu. Rozsah musí být explicitní, jinak čeština tiše zastará.

- [x] **A** — EN kanonická; CS plná pro README, průvodce, tutoriály, ADR a devlog; API reference jen EN — ✅ **doporučeno** · **zvoleno**
  - Přepínač jazyka v README i na docs webu. API reference zůstává v rustdoc (EN), protože se generuje z kódu. CS průvodce se aktualizují při každém milníku. Doporučeno.
- [ ] **B** — Plná parita EN + CS včetně API referencí
  - Nejlepší pro české vývojáře, ale udržet při rychlém vývoji je nereálné bez placeného překladu; hrozí, že jedna verze zastará.
- [ ] **C** — Nejdřív jen EN, CS doplníme po 0.1
  - Nejrychlejší, ale v rozporu s tvým požadavkem na dvojjazyčnost od začátku.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q85 — Jaký standard dokumentace v kódu?

*vyber jednu*

**Proč to řešíme:** „Kvalitně okomentováno" musí být vynutitelné, ne dobrovolné. Jinak se kvalita s přibývajícím kódem rozpadne – obzvlášť když hodně kódu píšou agenti.

- [x] **A** — deny(missing_docs) + modulové docs s invarianty + SAFETY u každého unsafe + příklady u veřejného API — ✅ **doporučeno** · **zvoleno**
  - Kompilátor vynutí dokumentaci veřejných věcí; clippy a rustdoc warnings jako chyby v CI. Každý modul má sekci „Invarianty" a „Proč takto". Doporučeno.
- [ ] **B** — Povinné docs jen u veřejného API, interní kód volně
  - Méně práce, ale interní moduly jsou přesně to, co bude chtít pochopit přispěvatel.
- [ ] **C** — Bez vynucení, jen doporučení
  - V praxi to znamená, že dokumentace vznikne částečně a nesouvisle.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q86 — Kde běžet CI?

*vyber jednu*

**Proč to řešíme:** macOS runnery u GitHubu jsou drahé a pomalé na start; Linux je levný. Máme ale vlastní M4 – můžeme ho použít jako self-hosted runner a ušetřit měsíčně nemalé peníze a mít realistické benchmarky.

- [x] **A** — Self-hosted M4 runner pro macOS + GitHub Linux pro rychlé testy — ✅ **doporučeno** · **zvoleno**
  - Nejlepší poměr cena/výkon a realistické měření na cílovém hardware. Vyžaduje údržbu runneru a zabezpečení (public repo = riziko zneužití runneru).
- [ ] **B** — Jen GitHub-hosted macOS runnery
  - Bez údržby, ale drahé (minuty se počítají) a pomalé; benchmarky budou zašuměné.
- [ ] **C** — Lokální CI (skripty) + příležitostné ruční běhy
  - Nejlevnější, ale žádná viditelná aktivita a snadno se zapomene testovat.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q87 — Jak hlídat výkonnostní regrese?

*vyber jednu*

**Proč to řešíme:** Bez bran se výkon zhoršuje po malých krocích a nikdo si toho nevšimne. Zároveň příliš přísné brány na PR zpomalují vývoj a vytvářejí falešné poplachy.

- [x] **A** — Criterion + vlastní harness: PR rychlé kontroly, nightly plná matice s publikovanou historií — ✅ **doporučeno** · **zvoleno**
  - PR brzdí jen hrubé regrese (např. >5 %), jemné změny řeší nightly report. Historie je veřejná. Doporučeno.
- [ ] **B** — Jen nightly, PR bez bran
  - Rychlý vývoj, ale regrese se objeví pozdě a špatně se hledá viník.
- [ ] **C** — Ruční měření před release
  - Nejjednodušší, ale v praxi se na něj zapomene.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q88 — Jaké release artefakty a signing?

*vyber jednu*

**Proč to řešíme:** Nepodepsaná binárka na macOS znamená děsivé varování a nedůvěru. Pro veřejný projekt, který má vypadat profesionálně, je codesign + notarizace nutnost – a je to práce na hodiny, ne dny.

- [ ] **A** — Podepsané a notarizované binárky, source tarball, crates.io, SBOM — ✅ **doporučeno**
  - Profesionální dojem a méně problémů u uživatelů. Vyžaduje Apple Developer account (99 USD/rok) a GH Actions workflow.
- [x] **B** — Nepodepsané zipy + source · **zvoleno**
  - Bez nákladů, ale každý uživatel musí obcházet Gatekeeper.
- [ ] **C** — Jen zdrojový kód, binárky ať si každý zkompiluje
  - Nejjednodušší, ale výrazně snižuje počet lidí, kteří si projekt vyzkouší.
- [ ] **jiné** — ______________________

**Zvoleno:** B · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q89 — Jak postavit dokumentační web?

*vyber jednu*

**Proč to řešíme:** Dokumentace musí být čitelná, prohledávatelná, verzovaná a dvojjazyčná. mdBook je standard Rust ekosystému, snadno se generuje v CI a dobře se v něm dělá přepínač jazyka.

- [x] **A** — mdBook (EN + CS) na GitHub Pages, verzované podle releasů, s přepínačem jazyka — ✅ **doporučeno** · **zvoleno**
  - Rychlé, standardní, snadno automatizovatelné; rustdoc pro API. Doporučeno.
- [ ] **B** — Docusaurus / Next.js
  - Hezčí a flexibilnější, ale je to frontendový projekt navíc a pomalejší build.
- [ ] **C** — Jen README + rustdoc
  - Nejmenší práce, ale pro „podrobnou dokumentaci" to nestačí.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

---

<a id="18-proces-agenti-pluginy-a-definice-hotoveho"></a>

## 18. Proces: agenti, pluginy a definice hotového

Tohle je sekce, která rozhoduje, jestli projekt skutečně dojde do konce. Já budu fungovat jako Lead: držím architekturu, rozhodnutí, review a integraci. Subagenti v Agent Teamu dostanou přesně vymezené moduly a write scopes, aby si nešlapali po souborech.

Klíčové pravidlo: **agent nikdy nedodá kód bez testů a bez dokumentace a nikdy nesloučí sám.** Lead reviewuje každý merge, spouští brány a hlídá, že rozhodnutí z briefingu se dodržují.

### Q90 — Jak má vypadat tým agentů?

*vyber jednu*

**Proč to řešíme:** Příliš málo agentů = pomalý postup a dlouhé blokace. Příliš mnoho = konflikty, nekonzistence a reviews, která nestíhám. Role musí kopírovat hranice crates, ne libovolné rozdělení.

- [x] **A** — 4 stálí agenti: core/arch, layout+text, paint+GPU, JS bindingy+API — ✅ **doporučeno** · **zvoleno**
  - Každý vlastní svůj crate a má vlastní testy. Lead dělá architekturu, review, integraci, dokumentaci a release. Doporučeno.
- [ ] **B** — 2–3 agenti, více práce na každém
  - Méně koordinace, ale menší paralelizace a pomalejší pokrok.
- [ ] **C** — 6–8 agentů podle modulů
  - Maximální paralelizace, ale velké riziko nekonzistence a přetíženého review; vhodné až po 0.1.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q91 — Jak přísné mají být merge gates?

*vyber jednu*

**Proč to řešíme:** V agentním vývoji je snadné vygenerovat velké množství kódu, které „funguje" na jeden případ. Brány jsou jediná obrana proti tomu, aby se do mainu dostal neotestovaný chaos.

- [x] **A** — Každý PR: CI zelené, testy, clippy -D warnings, docs, review Leadem, žádný direct push do main — ✅ **doporučeno** · **zvoleno**
  - Pomalé, ale jediné bezpečné. Zahrnuje povinný popis „Co/Proč/Dopad/Měření". Doporučeno.
- [ ] **B** — Agenti mohou mergovat sami po zeleném CI
  - Rychlejší, ale bez review se budou opakovat stejné chyby a architektura se rozejde.
- [ ] **C** — Lidský review u každého PR
  - Nejvyšší kvalita, ale při tomto tempu nereálné.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q92 — Které interní pluginy do harnessu máme postavit?

*vyber vše, co platí*

**Proč to řešíme:** Nástroje rozhodují o rychlosti celého projektu. Několik dobrých pluginů ušetří víc času než další funkce v jádře – a zároveň ukážou, že umíme vyvíjet i nástroje.

- [x] **A** — WPT runner (spuštění subsetu, přehled pass rate, diff proti předchozímu běhu) — ✅ **doporučeno** · **zvoleno**
  - Bez toho nebudeme vědět, jak jsme na tom se specifikací. Nejvyšší priorita.
- [x] **B** — Reftest / screenshot diff (generování, porovnání, vizuální náhled) — ✅ **doporučeno** · **zvoleno**
  - Páteř vizuální kvality; okamžitě ukáže regrese v renderu.
- [x] **C** — Bench tracker (historie criterions, Speedometer, paměť, energie + graf) — ✅ **doporučeno** · **zvoleno**
  - Veřejný dashboard a obrana proti regresím.
- [x] **D** — Offline spec index (WHATWG/MDN/WPT + rustdoc, fulltextové hledání) · **zvoleno**
  - Zrychlí práci agentů i moji; snižuje halucinace o specifikaci.
- [x] **E** — ADR / decision log generátor (z briefingu do DECISIONS.md) · **zvoleno**
  - Udrží rozhodnutí dohledatelná a konzistentní.
- [x] **F** — Crate audit dashboard (verze, licence, poslední release, CVEs) · **zvoleno**
  - Užitečné pro supply chain; lze nahradit cargo-deny + Dependabotem.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q93 — Jak využívat memory a graphify?

*vyber jednu*

**Proč to řešíme:** Kontext se ztrácí mezi sessions a mezi agenty. Memory drží rozhodnutí a konvence; graphify drží architekturu a odhalí, když se kód rozejde s návrhem.

- [x] **A** — Memory pro rozhodnutí, konvence a benchmarky; graphify na začátku každého milníku pro kontrolu architektonického driftu — ✅ **doporučeno** · **zvoleno**
  - Levé a dlouhodobě udržitelné. Každé rozhodnutí z briefingu se uloží jako memory a ADR. Doporučeno.
- [ ] **B** — Jen memory
  - Stačí na kontext, ale neodhalí strukturální dluh.
- [ ] **C** — Jen graphify
  - Dobré pro strukturu, ale ztrácíme rozhodnutí a jejich důvody.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q94 — Co musí splnit 0.1.0, aby se vydala?

*vyber vše, co platí*

**Proč to řešíme:** Definice hotového musí být binární a veřejná. Jinak se release posouvá donekonečna nebo se vydá něco, co neumí to, co jsme slíbili.

- [x] **A** — Korpus: definovaný seznam webů projde vizuálním testem s dokumentovanými výjimkami — ✅ **doporučeno** · **zvoleno**
  - Hlavní důkaz použitelnosti.
- [x] **B** — Benchmarky: změřená a publikovaná matice (Speedometer, MotionMark, paměť, energie, start) — ✅ **doporučeno** · **zvoleno**
  - Hlavní důkaz výkonu.
- [x] **C** — WPT: dosažený a publikovaný pass rate pro definovaný subset — ✅ **doporučeno** · **zvoleno**
  - Hlavní důkaz správnosti.
- [x] **D** — Dokumentace EN+CS dle Q84 a Q85, včetně architektury a průvodce přispíváním — ✅ **doporučeno** · **zvoleno**
  - Hlavní důkaz, že projekt může použít někdo jiný.
- [x] **E** — Stabilita: 24 h fuzz bez nálezu, žádná známá soundness chyba, memory testy bez úniků — ✅ **doporučeno** · **zvoleno**
  - Hlavní důkaz robustness.
- [x] **F** — Release: podepsané binárky, changelog, demo video, blog post — ✅ **doporučeno** · **zvoleno**
  - Hlavní důkaz, že projekt žije.
- [x] **G** — Žádná otevřená P0/P1 chyba a všechna ADR schválená · **zvoleno**
  - Procesní hygiena.
- [ ] **jiné** — ______________________

**Zvoleno:** A, B, C, D, E, F, G · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

### Q95 — Co se má stát hned po tvém vyplnění briefingu?

*vyber jednu*

**Proč to řešíme:** Briefing bez následné akce je jen dokument. Tato otázka potvrzuje, že rozumím zadání: nejdřív rozhodnutí, pak bootstrap, pak agenti.

- [x] **A** — DECISIONS.md + bootstrap repozitáře + M0 + spawn agentů — ✅ **doporučeno** · **zvoleno**
  - Z odpovědí vygeneruji ADR, založím repo se strukturou, CI a dokumentací, postavím walking skeleton a rozdělím práci agentům. Doporučeno.
- [ ] **B** — Nejdřív mi pošli revidovaný plán ke schválení
  - Jeden mezikrok navíc; vhodné, pokud chceš mít nad bootstrapem větší kontrolu.
- [ ] **C** — Ještě předtím hlubší technický design (detailní spec modulů)
  - Delší příprava a méně rizika, ale odsouvá první viditelný výsledek.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q96 — Co má být hotové za prvních 14 dní (M0)?

*vyber jednu*

**Proč to řešíme:** První dva týdny určují tempo a důvěryhodnost. Walking skeleton – něco, co jde spustit a vidět – je lepší než dokonalý design dokument.

- [x] **A** — Walking skeleton: repo, CI, workspace, docs web, ADR, wgpu/vello trojúhelník, screenshot harness, WPT runner — ✅ **doporučeno** · **zvoleno**
  - Vše propojené a spustitelné, i když prázdné. Doporučeno.
- [ ] **B** — Detailní architektonický dokument (100+ stran) před kódem
  - Méně rizika pozdějších změn, ale dva týdny bez výsledku a riziko „analýza paralýzou".
- [ ] **C** — Rovnou HTML parser a DOM
  - Nejrychleji k nějakému kódu, ale bez CI a harnessu se kvalita neudrží.
- [ ] **jiné** — ______________________

**Zvoleno:** A · *shoda s doporučením*

**Poznámka:** ______________________________________________

### Q97 — Jak často komunikovat pokrok?

*vyber jednu*

**Proč to řešíme:** Chceš vidět, že projekt žije. Pravidelný rytmus je nejlepší lék proti „tichému" projektu, který se měsíce neozve.

- [ ] **A** — Týdenní devlog + měsíční benchmark report + release notes u každé verze — ✅ **doporučeno**
  - Udržitelné a čitelné. Doporučeno.
- [ ] **B** — Denní krátké statusy
  - Maximální přehled, ale vysoká režie a brzy se z toho stane šum.
- [x] **C** — Jen při milnících · **zvoleno**
  - Nejmenší režie, ale mezi milníky projekt působí mrtvě.
- [ ] **jiné** — ______________________

**Zvoleno:** C · *⚠ odchylka od doporučení*

**Poznámka:** ______________________________________________

---

## Příloha A — Rychlá cesta (13 otázek, které rozhodují nejvíc)

Pokud nechceš vyplňovat všech 99 otázek, odpověz na těchto 13 a zbytek nech na doporučených hodnotách. Já je potom potvrdím v DECISIONS.md jako „přijato implicitně".

- **Q01** — Co je výstupem verze 0.1? _(doporučeno: A)_
- **Q04** — Jaká je vlajková věta projektu (jedna, do README)? _(doporučeno: E)_
- **Q07** — Jakou úroveň ambice má v0.1 mít? _(doporučeno: B)_
- **Q08** — Jaký je tvrdý časový horizont do 0.1.0? _(doporučeno: C)_
- **Q12** — Jakou licenci bude mít náš kód? _(doporučeno: A)_
- **Q17** — Jaký procesní model má mít 0.1? _(doporučeno: A)_
- **Q22** — Přes jaké API mluvit s GPU? _(doporučeno: A)_
- **Q39** — CSS: kaskáda a výpočet stylů – postavit na čem? _(doporučeno: A)_
- **Q62** — Který JavaScript engine? _(doporučeno: B)_
- **Q67** — Která Web API musí 0.1 mít? _(doporučeno: A,B,C,D,E,F,G,H,I,J,K,L)_
- **Q84** — Jaký rozsah češtiny v dokumentaci? _(doporučeno: A)_
- **Q90** — Jak má vypadat tým agentů? _(doporučeno: A)_
- **Q95** — Co se má stát hned po tvém vyplnění briefingu? _(doporučeno: A)_

## Příloha B — Odpovědní arch

Zkopíruj a vyplň. Formát: `Q01 = A`, u vícenásobných `Q40 = A,B,C,D`.

```text
# 1. Produkt, korpus a měřítko úspěchu
Q01 = 
Q02 = 
Q03 = 
Q04 = 
Q05 = 
Q06 = 
# 2. Realita, ambice a časový horizont
Q07 = 
Q08 = 
Q07b = 
Q09 = 
Q10 = 
Q11 = 
# 3. Licence, veřejnost a podoba repozitáře
Q12 = 
Q13 = 
Q14 = 
Q15 = 
# 4. Architektura a hranice systému
Q16 = 
Q17 = 
Q18 = 
Q19 = 
Q20 = 
Q21 = 
# 5. Apple Silicon, GPU a vlákna
Q22 = 
Q22b = 
Q23 = 
Q24 = 
Q25 = 
Q26 = 
# 6. Paměťová strategie
Q27 = 
Q28 = 
Q29 = 
Q30 = 
# 7. Síťová vrstva
Q31 = 
Q32 = 
Q33 = 
Q34 = 
Q35 = 
# 8. HTML, DOM a CSS
Q36 = 
Q37 = 
Q38 = 
Q39 = 
Q40 = 
Q41 = 
Q42 = 
Q43 = 
Q44 = 
# 9. Layout engine
Q45 = 
Q46 = 
Q47 = 
Q48 = 
Q49 = 
Q50 = 
# 10. Text, fonty a typografie
Q51 = 
Q52 = 
Q53 = 
Q54 = 
Q55 = 
Q56 = 
# 11. Vykreslování, kompozice a vizuální kvalita
Q57 = 
Q58 = 
Q59 = 
Q60 = 
Q61 = 
# 12. JavaScript, WebAssembly a bindingy
Q62 = 
Q63 = 
Q64 = 
Q65 = 
Q66 = 
# 13. Web API surface a formuláře
Q67 = 
Q68 = 
Q69 = 
Q70 = 
# 14. Měření: jak prokážeme, že jsme dobří
Q71 = 
Q72 = 
Q73 = 
Q74 = 
Q75 = 
Q76 = 
# 15. Bezpečnost, sandbox a soukromí
Q77 = 
Q78 = 
Q79 = 
Q80 = 
# 16. AI a agentní vrstva
Q81 = 
Q82 = 
Q83 = 
# 17. Dokumentace, CI a release
Q84 = 
Q85 = 
Q86 = 
Q87 = 
Q88 = 
Q89 = 
# 18. Proces: agenti, pluginy a definice hotového
Q90 = 
Q91 = 
Q92 = 
Q93 = 
Q94 = 
Q95 = 
Q96 = 
Q97 = 
```

## Příloha C — Co se stane po vyplnění

1. Z odpovědí vygeneruji `DECISIONS.md` (ADR-001…ADR-nnn) s odůvodněním a dopady.
2. Založím veřejný repozitář: workspace, CI, dokumentaci EN+CS, šablony a roadmapu.
3. Postavím M0 walking skeleton a rozdělím práci agentům v Agent Teamu.
4. Od každého milníku bude existovat měřitelný výstup (screenshot, benchmark, WPT číslo).
