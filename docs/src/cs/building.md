# Sestavení

## Požadavky

- macOS na Apple Silicon (M1 nebo novější)
- Rust stable 1.85+ s `rustfmt` a `clippy`
- Xcode command line tools (kvůli hlavičkám Metal SDK)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal -c rustfmt -c clippy
```

## Příkazy

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --open
```

## Metal probe

První věc, kterou M0 dokazuje, je, že vrstva Metal bindingu na tomhle stroji funguje:

```bash
cargo run -p jkva-platform --bin metal-probe
# Metal device: Apple M4
# Apple9 family: yes
```

Když vypíše `Metal is not available`, engine se pořád dá zkompilovat a jeho CPU testy spustit, ale GPU cesta není dostupná.

## CI

- macOS job: formátování, clippy, testy a rustdoc na Apple Silicon.
- Linux job: `cargo check` přenositelných cratí (`jkva-platform` a `jkva-shell` jsou jen pro macOS). Kontrolují se tu i oba nástroje z `tools/`.
- Docs job: sestaví tuhle knihu mdBookem, takže špatný `book.toml` nebo `SUMMARY.md` zastaví změnu, ne čtenáře.

Self-hosted M4 runner se používá pro benchmarky, ne pro každý pull request.
