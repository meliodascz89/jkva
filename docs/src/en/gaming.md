# Gaming platform

Gaming is a first-class goal of 0.1, not a later add-on (ADR-026). The engine must run demanding browser games at low latency and expose the GPU honestly, while staying safe by default.

## What "gaming" means here

A game in JKva is a web application that uses:

- WebGPU (preferred) or WebGL for rendering,
- WASM for simulation and game logic,
- `requestAnimationFrame` with a predictable frame cadence,
- gamepad, pointer lock, fullscreen and low-latency audio,
- UDP (through Tokio) or WebRTC data channels for multiplayer — **only after the user grants network access to that application and origin**.

## Non-goals

- Running native CS2 or any Steam title. JKva is a browser engine, not a compatibility layer for native games.
- Cloud gaming or streaming.
- Anti-cheat integration.

"CS2-class" means a web game with comparable load: WebGPU, WASM and a low-latency network path — not the native executable.

## Security model for UDP

UDP is not a general browser capability:

1. Default deny. No socket exists until an explicit grant.
2. A grant is per application **and** per origin, shown to the user, revocable, and scoped to the session.
3. Grant revokes when the application closes; nothing listens in the background.
4. Raw sockets and privileged ports are impossible; datagram size, rate and total volume are quota-limited.
5. The network process is sandboxed and separate from the renderer; every new channel passes the security review from ADR-016.

## Performance targets

Measured in our own benchmark, published with every milestone:

| Metric | Target |
|---|---|
| frame time p95 | ≤ 8 ms at 1080p on Apple Silicon |
| input-to-photon latency p95 | ≤ 2 frames |
| GPU utilisation | visible through timestamp queries, not guessed |
| comparison | the same scenes measured against Chrome and Safari |

A number without the scene, resolution, hardware and build is not a measurement; the benchmark harness records all four.

## Measured baseline (2026-10-26, Apple M4)

`cargo run --release -p gfx-bench -- --frames 120 --quads 4096 --width 1920 --height 1080`

| Series | p50 | p95 |
|---|---:|---:|
| CPU submit | 0.129 ms | 0.153 ms |
| GPU completion (command-buffer wait) | 1.979 ms | 3.647 ms |
| GPU execution (`GPUStartTime`..`GPUEndTime`) | 0.912 ms | 1.221 ms |

Capabilities: Apple M4, `maxBufferLength` 13,639.7 MiB, recommended working set 18,186.2 MiB, Apple7/8/9 and Metal4, unified memory, timestamp counters.

**This is not a game frame.** The scene is 4096 draw calls over 1920×1080 with no layout, style, JS, WASM, compositing, presentation or input, and frames are fully serialized. It is therefore neither evidence that the ADR-026 p95 ≤ 8 ms target is met nor comparable to Chrome/Safari until the same scene runs in all three engines.

### Engine frame (paint + submit)

`cargo run --release -p engine-bench -- --page examples/cards.html --frames 120 --width 800 --height 600`

| Metric | p50 | p95 |
|---|---:|---:|
| One frame (scene build + GPU wait) | 0.51 ms | 2.46 ms |

Scope: layout and style are static (built once), the measured frame rebuilds the scene — backgrounds, borders, glyph shaping and rasterization — uploads it and waits for the GPU, with no CPU readback. After glyph-texture, font and shaped-run caches the same frame dropped from 8.05 to about 0.51 ms p50 (10.81 to 2.46 ms p95, median of five runs); it is still a static page, not a game scene, and not yet comparable to Chrome.

### Game frame (animated sprites, damage)

`cargo run --release -p engine-bench -- --game --sprites 2000 --frames 120 --width 800 --height 600 --mode incremental|full`

| Mode | p50 | p95 | Average drawn items |
|---|---:|---:|---:|
| Damage (incremental) | 0.284 ms | 2.16 ms | 215 |
| Full redraw | 0.544 ms | 2.45 ms | 2000 |

2000 sprites, 100 of them moving, 800×600, median of five runs. Damage rendering keeps the target between frames, loads it instead of clearing, and draws only the items intersecting the damage rectangles — 9.3x fewer draws and about 1.9x better p50. The p95 is dominated by GPU fill and is nearly unchanged, which is the honest limit of this path until a real compositor with layers exists. There is still no JS, WASM or layout in this loop.

## GPU command ABI

The renderer does not talk to Metal directly. It builds a typed `CommandList`
(create buffer/texture, write, begin pass, draw, end pass), validates it against
the negotiated `DeviceCaps` — buffer and texture limits, balanced passes, draw
budget, resource existence — and encodes it into a versioned frame. The GPU
process decodes, validates again and executes; a foreign version is rejected
instead of half-executed.

This is the seam a Metal backend and a future WebGPU-style API share. The current
surface is deliberately small: buffers, textures, one render pass and draws.
Pipelines, bind groups, samplers and compute come next. The crate is
`jkva-gpu`; it is portable and covered by the Linux CI job.

## Status

The design is in ADR-026. Done: the permission-gated UDP transport in `jkva-net` (Tokio, default deny, grant/revoke, quotas) and the fixed-timestep `FrameClock` in `jkva-core` (capped simulation steps, interpolation alpha, rate changes). In progress: the GPU capability probe and frame-time harness in `jkva-gfx`. Gamepad input is a typed message in `jkva-ipc` protocol v2.
