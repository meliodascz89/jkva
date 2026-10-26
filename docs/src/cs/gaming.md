# Herní platforma

Herní výkon je první třída 0.1, ne přílepek (ADR-026). Engine musí umět náročné browserové hry s nízkou latencí a poctivě vydat GPU, ale zároveň zůstat bezpečný defaultně.

## Co tady „hraní“ znamená

Hra v JKva je webová aplikace, která používá:

- WebGPU (preferovaně) nebo WebGL pro rendering,
- WASM pro simulaci a herní logiku,
- `requestAnimationFrame` s předvídatelným pacingem,
- gamepad, pointer lock, fullscreen a nízko-latenční audio,
- UDP (přes Tokio) nebo WebRTC datové kanály pro multiplayer — **jen po výslovném grantu pro danou aplikaci a origin**.

## Non-goals

- Spouštět nativní CS2 nebo jiný Steam titul. JKva je browser engine, ne kompatibilní vrstva pro nativní hry.
- Cloud gaming nebo streaming.
- Anti-cheat integrace.

„CS2-class“ znamená webovou hru se srovnatelnou zátěží: WebGPU, WASM a nízko-latenční síť — ne nativní executable.

## Bezpečnostní model UDP

UDP není obecná schopnost prohlížeče:

1. Default deny. Žádný socket neexistuje, dokud není explicitní grant.
2. Grant je pro aplikaci **a** origin, uživatel ho vidí, může ho odvolat a platí jen pro session.
3. Zavřením aplikace grant zaniká; na pozadí nic neposlouchá.
4. Raw sockety a privilegované porty jsou nemožné; velikost datagramu, rychlost i objem jsou kvótované.
5. Síťový proces je v sandboxu a oddělený od rendereru; každý nový kanál prochází bezpečnostním review z ADR-016.

## Výkonnostní cíle

Měřené vlastním benchmarkem a publikované u každého milníku:

| Metrika | Cíl |
|---|---|
| frame time p95 | ≤ 8 ms při 1080p na Apple Silicon |
| input-to-photon latence p95 | ≤ 2 framy |
| využití GPU | viditelné přes timestamp queries, ne odhad |
| srovnání | stejné scény změřené proti Chrome a Safari |

Číslo bez scény, rozlišení, hardwaru a buildu není měření; harness eviduje všechny čtyři.

## Změřená baseline (2026-10-26, Apple M4)

`cargo run --release -p gfx-bench -- --frames 120 --quads 4096 --width 1920 --height 1080`

| Série | p50 | p95 |
|---|---:|---:|
| CPU submit | 0,129 ms | 0,153 ms |
| GPU completion (čekání na command buffer) | 1,979 ms | 3,647 ms |
| GPU execution (`GPUStartTime`..`GPUEndTime`) | 0,912 ms | 1,221 ms |

Capabilities: Apple M4, `maxBufferLength` 13 639,7 MiB, doporučený working set 18 186,2 MiB, Apple7/8/9 a Metal4, unified memory, timestamp counters.

**Tohle není game frame.** Scéna je 4096 draw callů na 1920×1080 bez layoutu, stylů, JS, WASM, kompozice, prezentace a vstupu a framy jsou plně serializované. Není to tedy důkaz splnění cíle p95 ≤ 8 ms z ADR-026 ani srovnání s Chrome/Safari, dokud stejná scéna neběží ve všech třech enginech.

### Engine frame (paint + submit)

`cargo run --release -p engine-bench -- --page examples/cards.html --frames 120 --width 800 --height 600`

| Metrika | p50 | p95 |
|---|---:|---:|
| Jeden frame (stavba scény + čekání na GPU) | 0,51 ms | 2,46 ms |

Rozsah: layout a styly jsou statické (postavené jednou), měřený frame znovu staví scénu — pozadí, rámečky, shaping a rasterizaci glyfů — nahraje ji a čeká na GPU, bez readbacku na CPU. Po cache glyfových textur, fontů a shaped runů spadl stejný frame z 8,05 na zhruba 0,51 ms p50 (z 10,81 na 2,46 ms p95, medián pěti běhů); pořád je to statická stránka, ne herní scéna, a zatím to není srovnatelné s Chrome.

### Herní frame (animované sprity, damage)

`cargo run --release -p engine-bench -- --game --sprites 2000 --frames 120 --width 800 --height 600 --mode incremental|full`

| Režim | p50 | p95 | Průměrně kreslených itemů |
|---|---:|---:|---:|
| Damage (inkrementální) | 0,284 ms | 2,16 ms | 215 |
| Plný překres | 0,544 ms | 2,45 ms | 2000 |

2000 spritů, 100 se pohybuje, 800×600, medián pěti běhů. Damage rendering drží target mezi framy, načítá ho místo clearu a kreslí jen itemy protínající damage rectangles — 9,3× méně draw callů a zhruba 1,9× lepší p50. p95 je dané fillem GPU a téměř se nemění, což je poctivý limit této cesty, dokud nebude skutečný kompositor s vrstvami. V loopu pořád není JS, WASM ani layout.

## GPU command ABI

Renderer nemluví s Metalem přímo. Staví typovaný `CommandList` (vytvoř buffer/texturu, zapiš, začni pass, draw, ukonči pass), validuje ho proti vyjednaným `DeviceCaps` — limity bufferů a textur, vyvážené passy, draw budget, existence resource — a zakóduje do verzovaného rámce. GPU proces ho dekóduje, znovu zvaliduje a vykoná; cizí verze se odmítne, místo aby se vykonala napůl.

To je šev, který sdílí Metal backend a budoucí WebGPU-style API. Aktuální plocha je záměrně malá: buffery, textury, jeden render pass a draw. Pipelines, bind groups, samplery a compute přijdou dál. Crate je `jkva-gpu`, je portable a běží v Linux CI.

## Permission gate

`jkva-api::permissions` je politická vrstva mezi aplikací a schopností. Hra požádá o `Udp` pro konkrétní origin; uživatel povolí **jednou** nebo **pro session**; neznámý požadavek je zamítnut a zamítnutí přebije dřívější grant. Granty jsou vázané na origin **a** aplikaci, session granty se při zavření aplikace mažou a originy se normalizují, přičemž plain HTTP je povoleno jen pro loopback vývoj.

Gate ukládá jen rozhodnutí. Nikdy neotevírá socket: `jkva-net` pořád odmítne bind bez svého grantu, takže chyba v promptu nemůže tiše zapnout síť.

## Metal execution

ABI už není jen papírový kontrakt: `jkva-gfx::GpuExecutor` vykonává validovaný `CommandList` na Metal — vytvoření a upload bufferu, render-target textury, jeden pass, bind vertex bufferu a draw. Test postaví trojúhelník celý přes ABI, vykoná ho, přečte target a zkontroluje červený pixel ve středu.

Pipeline `0` je scénová pipeline (position/uv/rgba vrcholy vzorkované z 1×1 bílé textury). Bind textur, další pipelines, instancing a compute jsou další kroky; executor odmítne, co neumí, místo aby tiše nic nenakreslil.

## Stav

Design je v ADR-026. Hotové: permission-gated UDP transport v `jkva-net` (Tokio, default deny, grant/revoke, kvóty) a fixed-timestep `FrameClock` v `jkva-core` (cap na kroky, interpolační alpha, změna rate). Rozpracované: GPU capability probe a frame-time harness v `jkva-gfx`. Gamepad vstup je typovaná zpráva v `jkva-ipc` protokolu v2.
