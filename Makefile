.PHONY: all run

# Ensure `cd` works properly by forcing everything to be executed in a single shell.
.ONESHELL:

all:
	cargo +nightly build --target wasm32-unknown-unknown --release
	cd target/wasm32-unknown-unknown/release
	wasm-bindgen reflections.wasm --out-dir . --no-typescript --browser --no-modules

run:
	$(info Open file://$(shell pwd)/src/main.html in your browser.)
	python3 src/webserver.py
