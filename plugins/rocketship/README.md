# rocketship

This is an example plugin for `termail`. 

## Compilation
Using `uv`, 

```bash
uv venv
source .venv/bin/activate
uv pip install componentize-py wasmtime
```

then to compile from the `rocketship` directory, you first need to generate the wasm bindings, then compile to wasm.
```bash
componentize-py -d ../../wit -w plugin bindings bindings/
componentize-py -d ../../wit -w plugin componentize --stub-wasi rocketship -o plugin.wasm
```

Then add `rocketship` to `termail.plugins` and write an email!