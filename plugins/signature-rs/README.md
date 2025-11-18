# signature-rs

A Rust plugin for termail that appends a signature to outgoing emails.

## Prerequisites

Ensure you have the Rust toolchain installed and the [WASI Preview 2](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip2.html) target added. This target allows Rust to compile directly to the Component Model, eliminating the need for adapters.

```bash
rustup target add wasm32-wasip2
```

## Compilation

Compile the plugin directly to a WebAssembly Component:

```bash
cargo build --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/signature_rs.wasm plugin.wasm
```

## Optimization (Recommended)

For significantly faster loading times within termail, pre-compile the component to a native system binary (`.cwasm`). This step requires the `wasmtime` CLI.

```bash
wasmtime compile plugin.wasm -o plugin.cwasm
```

## Usage

1.  Ensure `plugin.wasm` (or `plugin.cwasm`) is present in the `plugins/signature-rs/` directory.
2.  Add `"signature-rs"` to the `plugins` list in your `config.toml`.