# Architecture

JKva is a Cargo workspace with one crate per layer. Dependencies point only downwards; `jkva-core` knows nothing about the platform, and `jkva-platform` is the only crate that touches macOS frameworks.

| Crate | Owns |
|---|---|
| `jkva-core` | identifiers, arenas, scheduling primitives, diagnostics |
| `jkva-platform` | Metal device, thread QoS, signposts, clipboard, files |
| `jkva-net` | HTTP/1.1, HTTP/2, HTTP/3, TLS, cache, cookies |
| `jkva-dom` | HTML tree, events, shadow roots |
| `jkva-style` | CSS parsing, cascade, computed values, invalidation |
| `jkva-layout` | box tree, block/inline/float/table layout, fragmentation |
| `jkva-text` | shaping, font fallback, line breaking, glyph atlas |
| `jkva-paint` | display lists, stacking contexts, damage |
| `jkva-gfx` | command recording, Metal 4 rasterizer, layers, compositor |
| `jkva-js` | SpiderMonkey integration, Web IDL bindings, GC tracing |
| `jkva-api` | the Web API surface implemented on top of the engine |
| `jkva-engine` | document lifecycle, event loop, navigation, embedder API |
| `jkva-shell` | the reference demo shell (not the future browser) |

## Rules

- No crate above `jkva-platform` may depend on macOS frameworks.
- No `unsafe` without a `// SAFETY:` comment; Miri runs on the unsafe surface.
- Public items are documented; `missing_docs` is denied.
- Boundary changes are ADRs. See [`briefing/DECISIONS.md`](https://github.com/meliodascz89/jkva/blob/main/briefing/DECISIONS.md).
