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
- A version of make that supports `.ONESHELL` (e.g. GNU Make 3.83).

To build: `make`.
To run: `make run` (and open the given file in a web browser).
