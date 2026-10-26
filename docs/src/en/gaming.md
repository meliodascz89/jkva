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

## Status

The design is in ADR-026. The first implementation slice is the permission-gated UDP transport in `jkva-net` (Tokio, default deny, tests for grant/revoke). The GPU capability probe and frame-time harness follow in M2/M3.
