# rocketship

This is an example plugin for `termail` that demonstrates the async host API by calling back to the host and appending a rocket emoji to emails.

## Features

- Calls the termail host API using `invocation-id` for secure async operations
- Appends a 🚀 emoji to email content before sending

## Compilation

Using `uv`:

```bash
uv venv
source .venv/bin/activate
uv pip install componentize-py wasmtime
```

Then to compile from the `rocketship` directory, you first need to generate the wasm bindings, then compile to wasm.

> **Important:** Bindings must be generated in the **root directory** (`.`), not in a subdirectory like `bindings/`. The componentize-py runtime expects the `wit_world` package at the root level to properly patch host imports. 
If you find a way to fix this, so that we can use a `bindings/` directory, please submit a PR.

```bash
rm -rf bindings wit_world componentize_py_*
componentize-py -d ../../wit/main.wit -w plugin bindings .
componentize-py -d ../../wit/main.wit -w plugin componentize rocketship -o plugin.wasm

# OPTIONAL: Pre-compile to native code for MUCH faster loading (HIGHLY RECOMMENDED)
wasmtime compile plugin.wasm -o plugin.cwasm
```

The `.cwasm` file loads instantly vs ~5-10 seconds for `.wasm`. Termail will automatically prefer `.cwasm` if it exists.

## Usage

Add `rocketship` to the `plugins` list in `config.toml`:
```toml
plugins = ["rocketship"]
```

Then compose and send an email through termail. The plugin will call the host API and append a rocket emoji to your message.