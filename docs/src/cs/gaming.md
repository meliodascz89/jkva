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

## Stav

Design je v ADR-026. První implementační řez je permission-gated UDP transport v `jkva-net` (Tokio, default deny, testy grant/revoke). GPU capability probe a frame-time harness následují v M2/M3.
