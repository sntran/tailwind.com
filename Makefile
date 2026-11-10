# Build tailwind.com.
#
#   make            build/tailwind.com: one file for all the systems
#   make native     target/release/tailwind: a program for this computer
#   make wasm       build/tailwind.wasm: the program for WASI. With
#                   WASI_SDK_PATH (a wasi-sdk), the program has JavaScript
#                   for @plugin and @config.
#   make test       all the tests
#   make test-wasm  the WASI program against the record, in Node.js
#   make tlc        the TLA+ models of specs/, with TLC
#
# See docs/BUILDING.md and docs/TESTING.md.

BUILD ?= $(CURDIR)/build

.PHONY: all native wasm test test-rust test-cli test-wasm test-mix tlc

all: $(BUILD)/tailwind.com

$(BUILD)/tailwind.com: FORCE
	sh scripts/build.sh $@

native:
	cargo build --release

wasm:
	cargo build --release --target wasm32-wasip1
	mkdir -p $(BUILD)
	cp target/wasm32-wasip1/release/tailwind.wasm $(BUILD)/tailwind.wasm

test: test-rust test-cli tlc test-mix

test-rust:
	cargo test --release

# The program against the record of the official program: the plain
# cases, the cases with JavaScript plugins and configs, and the cases for
# --map. TAILWIND is the program to test (default: the program of
# "make native").
test-cli: native
	sh tests/cli.sh $${TAILWIND:-target/release/tailwind}
	CASES=tests/upstream/cli-js sh tests/cli.sh $${TAILWIND:-target/release/tailwind}
	CASES=tests/upstream/cli-map sh tests/cli.sh $${TAILWIND:-target/release/tailwind}

# The WASI program against the same records, in Node.js. The cases with
# JavaScript need a program from a build with WASI_SDK_PATH.
test-wasm: wasm
	node tests/wasi.mjs $(BUILD)/tailwind.wasm
	CASES=tests/upstream/cli-js node tests/wasi.mjs $(BUILD)/tailwind.wasm
	CASES=tests/upstream/cli-map node tests/wasi.mjs $(BUILD)/tailwind.wasm

test-mix: native
	TAILWIND_COM=$${TAILWIND:-$(CURDIR)/target/release/tailwind} mix test

tlc:
	sh scripts/tlc.sh

FORCE:
