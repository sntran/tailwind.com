#!/bin/sh
# Build the native NIF objects of tailwind_nif for BEAM.com (docs/NIF.md):
# OUT/libtailwind_nif.x86_64.o and OUT/libtailwind_nif.aarch64.o.
#
# Usage: sh scripts/nif.sh [OUT]          (default: build/nif)
#
# beam.com has no dlopen(). For erlang:load_nif("PATH", 0), it loads
# PATH.x86_64.o or PATH.aarch64.o (the CPU of the computer) into the
# running program (docs/NIFS.md of BEAM.com). tailwind_nif loads
# priv/native/libtailwind_nif, so these files go to
# priv/native/libtailwind_nif.x86_64.o and .aarch64.o of the application
# tailwind.
#
# An object is the crate native/nif with the feature `ape`, as for the
# file tailwind.com (scripts/cosmo.sh), with no main.c. It has one global
# name, tailwind_nif_nif_init, the init function of Rustler for the
# library tailwind_nif. Its other names are local, so they do not collide
# with the names of beam.com. The object uses only what each beam.com
# gives to an object: the enif_* functions (Rustler calls them by name on
# Linux) and the functions of the C library of beam_com_nif_libc.h. So
# watch.c has inotify only (TW_WATCH_INOTIFY_ONLY): on the other systems,
# a watch polls the files.
#
# The object has the store of the packages of the script engine, as the
# ZIP part of the file tailwind.com has it (TAILWIND_COM_EMBED_PACKAGES of
# native/tailwind/build.rs): each version of Tailwind CSS 4 works, with no
# other file. PACKAGES selects the versions, as for scripts/build.sh.
#
# The object has one section for each kind (native/nif/sections.ld), and
# no local name that no relocation uses: 14 MB, not 22 MB.
#
# Environment: COSMOCC, COSMOCC_VERSION, RUST_NIGHTLY and PACKAGES, as
# for scripts/build.sh. The Rust builds go to target/nif: the store in
# the program changes the crate tailwind, and the builds of
# scripts/build.sh stay in target.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$root/build/nif}
C=${COSMOCC:-$root/build/cosmocc}
nightly=${RUST_NIGHTLY:-nightly-2026-11-01}

log() { echo "nif.sh: $*" >&2; }

# shellcheck source=scripts/cosmo.sh
. "$root/scripts/cosmo.sh"
cosmo_check

work=$root/build/nif-work
rm -rf "$work"
mkdir -p "$work" "$OUT"
cd "$root"

export CARGO_TARGET_DIR="$root/target/nif"
if [ "${PACKAGES:-all}" != none ]; then
    package_store "$work/tailwindcss"
    log "The store of the packages of $store_count versions of Tailwind CSS goes into the objects"
    export TAILWIND_COM_EMBED_PACKAGES="$work/tailwindcss"
fi

for cpu in x86_64 aarch64; do
    d=$work/$cpu
    mkdir -p "$d"
    log "Building the Rust library for $cpu"
    rust_staticlib "$cpu" tailwind_nif tailwind_nif "$d/libtailwind_nif.a"
    log "Building the C files for $cpu"
    c_objects "$cpu" "$d" -DTW_WATCH_INOTIFY_ONLY
    # shellcheck disable=SC2086
    RUST_LINK_SCRIPT=$root/native/nif/sections.ld \
        sh "$root/scripts/rust_link.sh" "$C" "$cpu" tailwind_nif_nif_init "$d/nif.o" $objects "$d/libtailwind_nif.a"
    sh "$C/bin/$cpu-unknown-cosmo-objcopy" --strip-unneeded "$d/nif.o"
    # The object must have its entry, and no other global name.
    globals=$(sh "$C/bin/$cpu-unknown-cosmo-nm" -g --defined-only "$d/nif.o" | awk '{print $3}')
    if [ "$globals" != tailwind_nif_nif_init ]; then
        log "libtailwind_nif.$cpu.o has the global names: $globals"
        exit 1
    fi
    mv "$d/nif.o" "$OUT/libtailwind_nif.$cpu.o"
    log "wrote $OUT/libtailwind_nif.$cpu.o ($(wc -c < "$OUT/libtailwind_nif.$cpu.o") bytes)"
done
