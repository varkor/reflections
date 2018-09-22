# reflections
- `cargo +nightly build --target wasm32-unknown-unknown --release` (compile the Rust project)
- `wasm-bindgen reflections.wasm --out-dir . --no-typescript --browser --no-modules` (generate
the WebAssembly)
- `python3 src/webserver.py` (start the web server)
- Open `reflections/src/main.html` in a web browser