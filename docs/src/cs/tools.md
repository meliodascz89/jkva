# Nástroje

`tools/` obsahuje dva malé nástroje pro příkazovou řádku, které z otázky „funguje to ještě?“ udělají číslo, jež umí zkontrolovat stroj: `reftest-diff` pro referenční testy a `wpt-report` pro výsledky Web Platform Tests. Ani jeden nelinkuje proti enginu ani nesahá na Metal, takže se oba sestaví a poběží na Linuxu i na Apple Silicon.

## Jedna konvence, dvě úlohy

Oba nástroje se řídí stejnou konvencí návratových kódů a právě ta z nich dělá něco, co se dá vůbec použít v bráně:

| Kód | Význam |
|---|---|
| `0` | Co jsi měřil, je v toleranci. |
| `1` | Nástroj udělal svou práci a odpověď je ne: obrázky se liší, nebo je míra úspěšnosti pod cílem. |
| `2` | Nástroj svou práci udělat nemohl: špatné argumenty, nečitelný soubor, poškozený vstup. |

Rozdíl mezi `1` a `2` je celý smysl. Červený build pak umí říct „stránka se změnila“, nebo „nástroj spadl“ — a tyhle dva případy chce řešit někdo jiný.

## Jak je spustit

Když se chceš jen podívat, nech to udělat Cargo:

```bash
cargo run -p reftest-diff -- expected.png actual.png
cargo run -p wpt-report -- report.json
```

Dvě pomlčky `--` oddělují argumenty Carga od argumentů programu. V CI sestav binárky jednou a volej je přímo:

```bash
cargo build --release -p reftest-diff -p wpt-report
./target/release/reftest-diff expected.png actual.png
```

## reftest-diff

Referenční test porovnává screenshot vykreslený enginem s očekávaným obrázkem. `reftest-diff` odpovídá na jedinou otázku, která u té dvojice má smysl: kolik pixelů se doopravdy liší a jak moc.

```text
reftest-diff <expected.png> <actual.png> [--threshold N] [--max-ratio R] [--out diff.png]
```

| Přepínač | Výchozí | Význam |
|---|---|---|
| `--threshold N` | `0` | Tolerance na kanál v rozsahu `0`–`255`. Pixel se počítá jako jiný, když je jeho největší rozdíl kanálu **větší než** `N`, takže `--threshold 4` pořád přijme rozdíl přesně 4. |
| `--max-ratio R` | `0.0` | Podíl lišících se pixelů, který ještě projde. `--max-ratio 0.01` pustí dvojici, kde se liší až 1 % pixelů. |
| `--out diff.png` | nic se nezapíše | Zapíše diff obrázek: skutečný screenshot s každým lišícím se pixelem obarveným červeně. |

Srovnání je záměrně přímočaré. Žádná percepční metrika, žádná heuristika na anti-aliasing a žádné „skoro to sedí“: jakmile největší rozdíl kanálu přesáhne práh, pixel se liší. Právě ta nekompromisnost je to, co drží výsledek reprodukovatelný ve chvíli, kdy očekávaný obrázek vykreslil jiný stroj.

Tady je skutečný běh. Aktuální screenshot má blok 50 × 50 pixelů namalovaný špatnou barvou:

```console
$ reftest-diff expected/hello.png actual/hello.png --max-ratio 0.001 --out hello-diff.png
diff image: hello-diff.png
800x600: 2500/480000 pixels differ (0.5208%), max delta 240, threshold 0
$ echo $?
1
```

Liší se 0,52 % obrázku a volající dovolil 0,1 %, takže běh skončí kódem `1`. Bez `--max-ratio` skončí taky kódem `1`, protože výchozí tolerance je nula lišících se pixelů. Proti stejnému obrázku vypíše `0/480000 pixels differ (0.0000%), max delta 0` a skončí kódem `0`.

Tři věci, které řádek s použitím neříká:

- **Oba obrázky musí mít stejné rozměry.** Nesoulad velikostí se ohlásí na stderr a běh skončí kódem `1`, ne `2`. Dvě různé velikosti jsou rozdíl uvnitř dvojice, ne rozbitý nástroj.
- **Přijímají se jen 8bitové PNG** — RGB, RGBA, odstíny šedi nebo odstíny šedi s alfou; paletové obrázky se nejdřív rozvinou. Šestnáctibitový referenční obrázek nástroj odmítne kódem `2`.
- **Přepínač `--help` neexistuje.** Když nástroj spustíš bez argumentů nebo s neznámým přepínačem, vypíše řádek s použitím na stderr a skončí kódem `2`.

## wpt-report

Runner Web Platform Tests zapisuje JSON report přepínačem `--log-wptreport`. `wpt-report` z něj udělá tabulku, kterou přečteš v code review: kolik subtestů prošlo, pro každý adresář nejvyšší úrovně a celkem.

```text
wpt-report <report.json> [--min-pass-rate R] [--top N]
```

| Přepínač | Výchozí | Význam |
|---|---|---|
| `--min-pass-rate R` | `0.0` | Nejnižší přijatelná míra úspěšnosti jako **zlomek** mezi `0` a `1`. `--min-pass-rate 0.95` znamená 95 %. |
| `--top N` | `12` | Kolik řádků s adresáři vypsat. Řádek `TOTAL` se vypíše vždycky. |

Běh nad malým reportem vypadá takhle:

```console
$ wpt-report report.json
directory        passed   failed  skipped pass rate
css                  13        3        0    81.25%
dom                  10        1        0    90.91%
infrastructure        2        1        0    66.67%
html                  3        0        1   100.00%
TOTAL                28        5        1    84.85%
$ echo $?
0
```

Adresáře se řadí podle počtu selhání od nejvyššího a shody podle názvu — nahoře v tabulce je tedy to, co je potřeba dodělat. Když si řekneš o bránu, stejný report neprojde:

```console
$ wpt-report report.json --min-pass-rate 0.95
wpt-report: pass rate 84.85% is below the target 95.00%
$ echo $?
1
```

Propadlá brána je pořád report, a proto tabulka jde na stdout v obou případech a jen verdikt na stderr.

Pravidla počítání je dobré znát dřív, než se začneš hádat s číslem:

- Test, který hlásí subtesty, se počítá **po subtestech**; test bez subtestů se počítá **jednou**, podle svého vlastního stavu.
- `PASS` a `OK` jsou úspěch. `FAIL`, `ERROR`, `TIMEOUT`, `CRASH` a `PRECONDITION_FAILED` jsou selhání. `SKIP` je přeskočeno.
- **Neznámý stav se počítá jako selhání**, takže runner, který si vymyslí nový stav, si míru úspěšnosti potichu nezvedne.
- Míra úspěšnosti je `passed / (passed + failed)`. Přeskočené subtesty jsou v tabulce vidět, ale do míry nevstupují, protože test, který nikdy neběžel, o enginu nic neříká. Když neběželo nic, je míra `0.00%`, ne `NaN`.

Poškozený vstup ukončí běh kódem `2` a jednou řádkou na stderr: chybějící soubor, nevalidní JSON nebo dokument bez pole `results`.

## Zatím to není brána

Oba nástroje jsou psané jako brány a oba kompiluje linuxový CI job; jejich unit testy běží v rámci `cargo test --workspace` na macOS. Ani jeden ale zatím není zapojený do workflow, které by umělo shodit pull request, protože není co měřit: repo nemá korpus referenčních obrázků ani běh WPT k sumarizaci. ADR-023 uvádí reference-test diff i WPT runner mezi CI pluginy, takže ber návratové kódy výše jako kontrakt, ke kterému nástroje směřují — ne jako slib, co dnes `main` vynucuje.

## reftest-runner

Vykreslí každé `*.html` v adresáři korpusu přes `jkva-shot` a porovná výsledek
s `expected/<název>.png`.

```bash
cargo run -p jkva-shell --bin jkva-shot -- examples/hello.html examples/expected/hello.png
cargo run -p reftest-runner -- examples          # ok / FAIL pro každou stránku
cargo run -p reftest-runner -- examples --update # obnoví zlaté obrázky
```

Návratové kódy: `0` vše sedí, `1` rozdíly nebo chybějící obrázky, `2` chyba.
Pomocí `JKVA_SHOT` lze použít jiný renderer; výchozí je `target/debug/jkva-shot`.
