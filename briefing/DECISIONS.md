# JKva 0.1 — rozhodnutí (ADR)

**Stav:** závazné · **Datum:** 2026-10-25 · **Zdroj:** [`ANSWERS.md`](ANSWERS.md) (99/99 odpovědí) · **Kontext:** [`BRIEFING-0.1.cs.md`](BRIEFING-0.1.cs.md)

Tento dokument je výstup briefingu. Každé ADR shrnuje jednu oblast, uvádí rozhodnutí, otázky a důsledky. Detaily a alternativy jsou v briefingu; zde je jen to, co platí.

## Souhrn parametrů

| Parametr | Hodnota |
|---|---|
| Produkt | jádro **JKva** jako Rust knihovna + demo shell; prohlížeč je samostatný pozdější projekt |
| Rozsah 0.1 | browser-grade engine (tabulky dle spec, Shadow DOM, OOPIF, multi-proces, média, platformní API) |
| Horizont | **36–48 měsíců** |
| Odhad rozsahu | **400–450 tis. řádků** |
| Tým | 4 stálí agenti + Lead; specialisté na uzavřené moduly |
| Licence | MIT OR Apache-2.0 (MPL-2.0 závislosti zůstávají závislostmi) |
| Veřejnost | od prvního commitu, Conventional Commits, changelog |
| JS engine | SpiderMonkey (`mozjs`) s přesným GC trasováním |
| GPU | přímý **Metal 4** + **vlastní GPU rasterizér** |
| Procesy | plný multi-proces (browser / renderer / GPU / net) + OOPIF |
| Paměť | systémový malloc, dokumentové a snímkové areny, soft cap, bfcache |
| Měření | Speedometer ≥ 25 (brána), WPT 55/80/75/40, footprint, energie, start |
| Dokumentace | EN kanonická + CS průvodce, `deny(missing_docs)` |

---

## ADR-001 — Produkt: knihovna + demo shell

**Rozhodnutí:** JKva je knihovna (crates `jkva-*`, binárka `jkva`). Součástí je tenký referenční shell pro demo, benchmarky a screencasty. Prohlížeč (okna, taby, procesy, historie, stahování, UI) je **samostatný projekt**, který vznikne později a jádro konzumuje přes veřejné API.
**Otázky:** Q01 = A, Q06 = A
**Důsledek:** veřejné API jádra je produkt; shell se nesmí stát produktem a nesmí do něj prosakovat logika prohlížeče.
**Riziko:** shell postupně bobtná. *Mitigace:* shell je samostatný crate s vlastním CI a nesmí být závislostí žádného `jkva-*` cratu.

## ADR-002 — Rozsah a horizont

**Rozhodnutí:** 0.1 je browser-grade engine, horizont **36–48 měsíců**, odhad 400–450 tis. řádků. Korpus: top-100 Tranco, top-1000, 20 interaktivních aplikací, dokumentace/long-form, e-shopy, zpravodajské servery, vlastní dream corpus a Speedometer workloads. Dream corpus se **zamrazí v M2**.
**Otázky:** Q02 = vše, Q07 = C, Q08 = D, Q07b = B, Q11 = A
**Důsledek:** žádné „na to nemáme čas" u tabulek, Shadow DOM, OOPIF, médií ani a11y. Řezy se dělají podle Q10 (tabulky → floaty → iframes → Shadow DOM → animace → workers → Wasm), ale pouze pokud se skutečně nestíhá.
**Riziko:** šířka korpusu vs. hloubka kvality. *Mitigace:* must-pass / best-effort seznamy a veřejný stav korpusu.

## ADR-003 — Měření a veřejná tvrzení

**Rozhodnutí:** vlajková věta „špičková vizuální kvalita při poloviční paměti". Veřejná tvrzení: paměť, energie, start, vizuální věrnost, benchmarková parita. **Speedometer 3.1 je brána s cílem ≥ 25**, WPT **55 % / DOM 80 / HTML 75 / CSS 40**. MotionMark se publikuje i jako deficit. Měří se na stejném M4, stejný den, interleaved A/B/C, mediány, bootstrap CI, `footprint -t`. Výsledky jdou na veřejný dashboard.
**Otázky:** Q03 = A–E, Q04 = E, Q71 = A, Q72 = A, Q73 = A–G,I, Q74 = A, Q75 = A, Q76 = A–D
**Důsledek:** každý milník má měřitelné číslo; žádné tvrzení bez metodiky a bez uvedení obětí (single-process by byl rychlejší, my máme multi-proces).
**Riziko:** benchmark marketing. *Mitigace:* předregistrace tvrzení před měřením, publikace všech běhů včetně regresí.

## ADR-004 — Licence, veřejnost, commity

**Rozhodnutí:** MIT OR Apache-2.0. Repo veřejné od prvního commitu, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY.md, šablony, roadmapa, Discussions, badges, auto release notes. Conventional Commits + generovaný CHANGELOG + PR šablona (Co/Proč/Dopad/Měření).
**Otázky:** Q12 = A, Q13 = A, Q14 = A, Q15 = vše
**Důsledek:** MPL-2.0 závislosti (`cssparser`, `selectors`, `stylo`, `mozjs`) zůstávají závislostmi; naše soubory zůstávají permisivní.
**Riziko:** MPL fork (pokud budeme muset patchovat) – změny v MPL souborech musí být zveřejněné.

## ADR-005 — Workspace a hranice cratů

**Rozhodnutí:** jeden Cargo workspace, jeden crate na vrstvu: `jkva-core`, `jkva-platform`, `jkva-net`, `jkva-dom`, `jkva-style`, `jkva-layout`, `jkva-text`, `jkva-paint`, `jkva-gfx`, `jkva-js`, `jkva-api`, `jkva-engine`, `jkva-shell`, plus `xtask`, `tools/`, `benches/`.
**Otázky:** Q16 = A
**Důsledek:** `jkva-core` nesmí záviset na macOS; `jkva-platform` je jediné místo s `objc2`/Metal. Linux CI díky tomu zůstává levné.

## ADR-006 — Procesy a embedding

**Rozhodnutí:** **plný multi-proces** — browser (UI/koordinace), renderer (dokument), GPU, network; IPC od M1, sandbox a crash recovery součástí 0.1. Embedding API: Rust API + malé stabilní C ABI pro okno, vstup a životní cyklus.
**Otázky:** Q17 = B, Q20 = C
**Důsledek:** přepisuje Q05 (multi-proces nebyl „mimo 0.1"). Renderer izolovaný seatbelt profilem; pád stránky nesmí shodit jádro. OOPIF (Q43) je přirozený důsledek.
**Riziko:** +4–6 měsíců a nejtěžší ladění. *Mitigace:* IPC kontrakty a fault-injection testy od M1; single-process režim jako dev fallback.

## ADR-007 — Vlákna, QoS a event loop

**Rozhodnutí:** actor/message-passing s explicitními prioritami (Interactive, UserInitiated, Utility, Background) mapovanými na QoS macOS. Event loop vlastní embedder (`pump/tick`), jádro má vnitřní async.
**Otázky:** Q18 = A, Q19 = A, Q24 = A
**Důsledek:** deterministické testy a snadné vložení do SwiftUI/AppKit; scroll a input nikdy nečekají na layout.

## ADR-008 — DOM a paměťová strategie

**Rozhodnutí:** vlastní DOM nad `html5ever` TreeSink, generační arena s typovanými indexy. Systémový malloc, dokumentová arena (reset při navigaci) + snímková arena pro layout/paint. Soft cap ~256 MB na dokument s evikcí, bfcache pro 1–2 dokumenty, rozpočet na dekódované obrázky.
**Otázky:** Q21 = A, Q27 = A, Q28 = A, Q29 = A, Q30 = A, Q38 = A
**Důsledek:** uvolnění O(1), žádné Rc cykly, předvídatelná paměť. Paměť se měří `footprint -t` včetně dětí.
**Riziko:** arena + multi-proces = sdílení přes shared memory; nutné navrhnout životnost bufferů před M2.

## ADR-009 — GPU, rasterizér a zero-copy

**Rozhodnutí:** přímý **Metal 4** (objc2-metal) a **vlastní GPU rasterizér** cest, AA, clipů a glyfů. Plná zero-copy pipeline přes UMA. Vello zůstává jen jako referenční srovnání pro testy, **není v produktové cestě** (Q22b ruší Q23).
**Otázky:** Q22 = B, Q22b = C, Q25 = B, Q23 = A (nahrazeno)
**Důsledek:** nejvyšší výkon a kontrola, ale macOS-only GPU cesta; Linux CI běží CPU/headless. Rasterizér je samostatný podobor – vyčlenit agenta a čas.
**Riziko:** +4–6 měsíců a riziko, že kvalita AA nebude od začátku srovnatelná. *Mitigace:* zlaté screenshoty proti Safari od M1, CPU fallback pro deterministické testy.

## ADR-010 — Platformy

**Rozhodnutí:** produktově macOS arm64; jádro musí jít zkompilovat a testovat na Linuxu (CI, headless). iOS, x86_64 a Windows až po 0.1.
**Otázky:** Q26 = A
**Důsledek:** žádné macOS-only API v `jkva-core`; platformní vrstva izolovaná.

## ADR-011 — Paměťová měření

**Rozhodnutí:** publishovaná metrika je **fyzický footprint** (`footprint -t -p … -j`), nikdy VSZ ani samotné RSS. Report: total, per-marginal-tab `(F20−F1)/19`, peak.
**Otázky:** Q29, Q30, Q73 = E
**Důsledek:** multi-proces se musí měřit včetně dětí, jinak vypadá lepší, než je.

## ADR-012 — Síť

**Rozhodnutí:** `hyper` + `rustls` s platform verifierem; **HTTP/3 (quinn/neqo) už v 0.1**; cache podle RFC 9111 (`http-cache-semantics`) s vlastní disk cache; cookies přes `cookie_store` s vlastním obalem (SameSite, CHIPS), systémová proxy, DoH později.
**Otázky:** Q31 = A, Q32 = A, Q33 = B, Q34 = A, Q35 = A
**Důsledek:** síť běží ve vlastním procesu (ADR-006); transport je vyměnitelný.
**Riziko:** `cookie_store` je od února bez commitů – počítej s forem.

## ADR-013 — Parsery a kaskáda

**Rozhodnutí:** `html5ever` (bez source spans), `cssparser`/`lightningcss` pro syntaxi, **vlastní kaskáda a computed values** nad řízenou sadou vlastností, `selectors` pro matching. V M0 proběhne týdenní spike Stylo jako rozhodovací bod; pokud vyhraje, mění se jen `jkva-style`.
**Otázky:** Q36 = A, Q37 = A, Q39 = A, Q41 = A
**Důsledek:** moderní syntaxe zdarma, kontrola nad invalidací, MPL jen v závislostech.
**Riziko:** ruční kaskáda je největší „neviditelná" práce; rozsah vlastností řídit daty o reálném výskytu, ne spekulací.

## ADR-014 — CSS rozsah, Shadow DOM, iframes

**Rozhodnutí:** plný rozsah včetně container queries, scroll-driven animací a anchor positioningu; **plná podpora Shadow DOM** (open/closed, slots, custom elements v1); **OOPIF** pro iframy.
**Otázky:** Q40 = vše, Q42 = A, Q43 = B
**Důsledek:** moderní komponentové weby a Speedometer jsou dosažitelné; cena je scoped styly, event retargeting a cross-process frame tree.

## ADR-015 — Formuláře a text

**Rozhodnutí:** vlastní CSS-renderované ovládací prvky s textovým editingem (výběr, kurzor, mazání, schránka, mrtvé klávesy, IME). `harfrust` na shaping, systémové fonty + malé fallbacky, glyfový atlas, Retina-first grayscale AA s gamma korekcí, variabilní fonty, COLRv1 a emoji.
**Otázky:** Q44 = A, Q51 = A, Q52 = A, Q53 = A, Q54 = A, Q55 = A, Q56 = A
**Důsledek:** formuláře a čeština fungují od začátku; žádné nativní widgety (vzhled by nešel stylovat).

## ADR-016 — Layout

**Rozhodnutí:** hybrid — `taffy` pro flex a grid, vlastní blok/inline/float/table; **tabulky podle plné specifikace**; základní floaty + BFC; **inkrementální layout od M1**; Rayon pro paralelní podstromy hned (s feature flagem pro deterministické testy); horizontal-tb, plné scroll kontejnery, sticky a fixed.
**Otázky:** Q45 = A, Q46 = C, Q47 = A, Q48 = A, Q49 = B, Q50 = A
**Důsledek:** RTL text se vykreslí správně, ale RTL layout stránky a vertikální psaní jsou mimo 0.1 – viditelná mezera, kterou dokumentujeme.
**Riziko:** Rayon v rané fázi = nedeterminismus; *mitigace:* `--jkva-single-thread` režim a deterministické snapshot testy.

## ADR-017 — Paint, efekty a média

**Rozhodnutí:** retained strom vrstev + display listy + damage tracking; automatické heuristiky povyšování vrstev; kvalita: AA, sRGB+P3, HDR, ostrý Retina text, stíny/blur/gradienty s ditheringem, 120 Hz pacing, backdrop-filter, věrnost referenci. Efekty: radius, shadow, gradienty, filtry, clip-path, masky, mix-blend, transitions. Média: **SVG, Canvas 2D, video přes AVFoundation i WebGL/WebGPU**.
**Otázky:** Q57 = A, Q58 = A, Q59 = vše, Q60 = A–E,G, Q61 = D
**Důsledek:** vizuální záběr na úrovni Safari; cena jsou měsíce práce na médiích a shaderech.

## ADR-018 — JavaScript a Wasm

**Rozhodnutí:** **SpiderMonkey (`mozjs`)** s TraceTrait přesným trasováním; bindingy generované z Web IDL; **wasmtime v 0.1**; ES moduly + import maps; workery v 0.1 (vynuceno Service Workers z Q67).
**Otázky:** Q62 = B, Q63 = A, Q64 = A, Q65 = B, Q66 = A (přepsáno Q67)
**Důsledek:** stejný JS jako Firefox ⇒ poctivé srovnání delta; žádné vlastní JIT riziko.
**Riziko:** `mozjs` API a build (prebuilt archivy) – pinovat verzi, CI cache.

## ADR-019 — Web API a platformní integrace

**Rozhodnutí:** plná sada pro korpus: DOM/CSSOM/events, fetch/XHR/WebSocket, history/location, storage, timery/rAF/observers, layout API, soubory/Blob/clipboard/crypto/matchMedia, Canvas 2D, **IndexedDB, Service Workers, WebRTC, geolokace, notifikace**; **plná VoiceOver integrace** hned.
**Otázky:** Q67 = vše, Q68 = A (přepsáno Q67), Q69 = B, Q70 = B
**Důsledek:** rozsah API je srovnatelný s prohlížečem; VoiceOver most je vazba na AppKit.
**Riziko:** Service Workers + WebRTC jsou samostatné subsystémy; *mitigace:* vyčlenit je jako moduly se specialistou (Q90).

## ADR-020 — Bezpečnost

**Rozhodnutí:** plný origin model + SOP + CORS; hardened runtime + seatbelt profil rendereru + JIT entitlement; **plná CSP Level 3**; povinné cargo-deny, Miri nad unsafe, cargo-fuzz 24 h a review citlivých cratů.
**Otázky:** Q77 = A, Q78 = A, Q79 = B, Q80 = A,C,D,F
**Důsledek:** bezpečnostní model je od M1, ne „až potom"; každý nový IPC kanál prochází review.

## ADR-021 — AI vrstva

**Rozhodnutí:** jádro exportuje sémantický strom (JSON: uzly, role, jméno, bbox, stav, akce) a akční API. **Žádné modely v jádře.** AI orchestrace patří do budoucího prohlížeče nebo sidecaru.
**Otázky:** Q81 = A, Q82 = A, Q83 = A
**Důsledek:** export je součástí API od M1, takže se nestane „přílepkem"; zároveň jádro zůstává rychlé a bezpečné.

## ADR-022 — Dokumentace

**Rozhodnutí:** EN kanonická; CS plná pro README, průvodce, tutoriály, ADR a devlog; API reference jen EN (rustdoc). `deny(missing_docs)`, modulové invarianty, SAFETY u unsafe, příklady u veřejného API. mdBook EN+CS na GitHub Pages.
**Otázky:** Q84 = A, Q85 = A, Q89 = A
**Důsledek:** dvojjazyčnost je reálná, ne slib; CS se aktualizuje při každém milníku.

## ADR-023 — CI, release, tým, proces

**Rozhodnutí:** self-hosted M4 runner + GitHub Linux; criterion s PR branami (>5 % regrese) a nightly maticí; **nepodepsané zipy + source** (odchylka); 4 stálí agenti + Lead; přísné merge gates (CI, testy, clippy, docs, review, žádný direct push); pluginy WPT runner, reftest diff, bench tracker, spec index, ADR generátor, crate audit; memory + graphify.
**Otázky:** Q86 = A, Q87 = A, Q88 = B, Q90 = A, Q91 = A, Q92 = vše, Q93 = A
**Důsledek:** kvalita a viditelnost aktivity; Gatekeeper bude u nepodepsaných buildů varovat.
**Riziko:** Lead je úzké hrdlo review. *Mitigace:* malé PR, šablony, automatické brány před review.

## ADR-024 — Definice hotového a další krok

**Rozhodnutí:** 0.1.0 se vydá, když: korpus projde vizuálním testem, **herní benchmark splňuje cíle ADR-026**, benchmark matice je publikovaná, WPT cíl splněn, dokumentace EN+CS hotová, 24 h fuzz bez nálezu a bez úniků, release + demo + blog, žádná P0/P1 a schválená ADR. První krok je M0 walking skeleton; komunikace při milnících (Q97 = C) s povinným release a reportem u každého milníku.
**Otázky:** Q94 = vše, Q95 = A, Q96 = A, Q97 = C
**Důsledek:** release je binární stav, ne dojem. M0 začíná okamžitě po tomto dokumentu.

## ADR-026 — Herní výkon jako první třída (0.1)

**Rozhodnutí:** 0.1 není jen browser-grade engine; je to zároveň **nízko-latenční herní platforma**. Herní výkon se měří od M2, ne až v M7, a je součástí Definition of Done 0.1.
- **Rozsah herní vrstvy:** WebGPU a WebGL, WASM/JS, gamepad, pointer lock, fullscreen, nízko-latenční audio, `requestAnimationFrame` s deterministickým pacingem, Performance API, WebRTC datové kanály a **UDP transport přes tokio**.
- **Souhlas:** UDP a jakákoli syrová síť jsou dostupné jen po **výslovném grantu pro každou aplikaci a origin zvlášť**. Default deny; grant je viditelný, odvolatelný a vázaný na session. Žádné privilegované porty, žádné raw sockety, žádný trvalý listener bez aktivní hry.
- **On-demand:** herní a síťové subsystémy se inicializují lazy při prvním grantu a při ukončení aplikace se zavřou. Nic neběží na pozadí „pro jistotu“.
- **Bezpečnost:** síťový proces v sandboxu, kvóty na datagramy a rychlost, validace adres, oddělení od rendereru, audit každého nového kanálu (ADR-016).
- **Měřitelné cíle:** frame time p95 ≤ 8 ms při 1080p na Apple Silicon v našem herním benchmarku; input-to-photon latence p95 ≤ 2 framy; stejné scénáře změřené proti Chrome a Safari; GPU timestamp queries a memory budget jako první-classmetry.
- **Non-goals:** spouštět nativní CS2 nebo jiné Steam hry, cloud gaming/streaming, anti-cheat. „CS2-like“ znamená webovou hru s podobnou zátěží (WebGPU, WASM, UDP), ne spuštění nativního titulu.
- **Rizika:** WebGPU ještě není hotové, UDP rozšiřuje attack surface, anti-cheat je mimo dosah, thermals na noteboocích. Odhad +3–4 měsíce nad rámec 36–48.

**Otázky:** nové Q100–Q104 (herní cíl, UDP consent, GPU budget, benchmark, non-goals)
**Důsledek:** každý milník od M2 musí kromě browserových testů doložit i herní metriku; bez ní není milník hotový.

## Rozpory mezi odpověďmi a jak jsou vyřešené

| Rozpor | Řešení |
|---|---|
| Q05 vylučuje multi-proces, Q17 ho zvolil | **Q17 vyhrává** – multi-proces je v 0.1 (ADR-006) |
| Q23 volí vello, Q22b volí vlastní rasterizér | **Q22b vyhrává** – vello jen referenční srovnání (ADR-009) |
| Q66 odkládá workery, Q67 zahrnuje Service Workers | **Q67 vyhrává** – workery musí být v 0.1 (ADR-018) |
| Q68 odkládá IndexedDB, Q67 ji zahrnuje | **Q67 vyhrává** – IndexedDB v 0.1 (ADR-019) |
| Q07 (24+) vs Q08 (18–24) vs Q07b (36–48) | **Q07b vyhrává** – 36–48 měsíců (ADR-002) |

## Fáze a odhad

| Fáze | Obsah | Odhad |
|---|---|---|
| M0 | repo, CI, dokumentace, ADR, IPC kontrakty, walking skeleton, první trojúhelník Metalem | 6 týdnů |
| M1 | HTML→DOM→CSS→blok/inline layout→text, screenshot a WPT harness, inkrementální seam | 4 měsíce |
| M2 | flex/grid, tabulky, floaty, obrázky, scroll, hit-testing, vrstvy, stacking contexts | 5 měsíců |
| M3 | vlastní GPU rasterizér, kompozitor, damage, efekty, barvy/HDR | 5 měsíců |
| M4 | mozjs + WebIDL bindingy + event loop + formuláře/IME + Shadow DOM | 5 měsíců |
| M5 | multi-proces, sandbox, OOPIF, CSP, crash recovery | 5 měsíců |
| M6 | média a platformní API: video, WebGL/WebGPU, IDB, SW, WebRTC, geo, a11y | 5 měsíců |
| M7 | výkon, paměť, energie, Speedometer, WPT, korpus | 5 měsíců |
| M8 | dokumentace EN+CS, 0.1.0, demo, blog | 3 měsíce |

Součet ≈ 38 měsíců + buffer = **36–48 měsíců**.

**Herní větev (ADR-026):** M2 input a frame pacing; M3 GPU renderer a timestampy; M4 WASM/JS; M5 sandbox síťového procesu; M6 WebGPU, UDP s grantem, WebRTC a gamepad; M7 herní benchmarky proti Chrome/Safari. Odhad +3–4 měsíce.

## Otevřená rizika

1. **Vlastní GPU rasterizér** – největší technické riziko; kvalita AA a výkon glyfů se musí měřit od M1 proti Safari.
2. **OOPIF + multi-proces** – cross-process frame tree a sdílené compositing surfaces; nejdražší architektonická část.
3. **Zero-copy + multi-proces** – sdílená paměť, životnost bufferů, synchronizace.
4. **WebRTC, Service Workers, video, WebGL/WebGPU** – čtyři samostatné subsystémy; každý potřebuje specialistu.
5. **VoiceOver most** – vazba na AppKit a dlouhé ladění.
6. **Rayon od začátku** – nedeterminismus; nutný single-thread režim pro testy.
7. **Review bottleneck** – 4 agenti generují víc kódu, než jeden Lead stíhá; nutné malé PR a automatické brány.
8. **Dokumentace v CS** – snadno zastará; aktualizace je součástí Definition of Done každého milníku.
