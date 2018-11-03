# reflections
Experiments in rendering nonaffine transformations, such as reflections and translations. This is
an early work-in-progress. An article exploring these concepts using the library here for rendering
will be forthcoming.

## Set-up instructions
(Note that these instructions may be incomplete. If something doesn't work, feel free to post an
issue or send a pull request.)

You'll need:
- The [Rust](https://www.rust-lang.org/) compiler.
- [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen).
- [Python 3](https://www.python.org/download/releases/3.0/).

To run:
- Compile the Rust library: `cargo +nightly build --target wasm32-unknown-unknown --release`.
- Generate the WASM bindings: `wasm-bindgen reflections.wasm --out-dir . --no-typescript --browser --no-modules`.
- Start the web server: `python3 src/webserver.py`.
- Open the Reflection Lab in a web browser (`reflections/src/main.html`).
