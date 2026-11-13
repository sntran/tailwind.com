#!/bin/sh
# Build tailwind.com: one executable file for Linux, macOS, Windows,
# FreeBSD, NetBSD and OpenBSD, on x86_64 and aarch64.
#
# Usage: sh scripts/build.sh [OUT]          (default: build/tailwind.com)
#
# The program is the Rust crate native/tailwind as a staticlib (the std of
# the target *-unknown-linux-musl), with the calls of native/tailwind/c_src/cosmo_rust.c,
# linked with cosmocc into one Actually Portable Executable. This is the
# method of BEAM.com for a library of Rust. scripts/cosmo.sh has the steps.
#
# Environment:
#   COSMOCC          Directory of an unpacked cosmocc (default build/cosmocc;
#                    this script downloads it when it is not there)
#   COSMOCC_VERSION  cosmocc release to download (default 4.0.2)
#   RUST_NIGHTLY     The nightly Rust for aarch64 (default nightly-2026-11-01)
#   CPUS             The CPUs of the file (default "x86_64 aarch64")
#   PACKAGES         The versions of Tailwind CSS whose npm package goes into
#                    the ZIP directory tailwindcss/ of the file, for the
#                    script engine: "all" (default: each version of the
#                    file PACKAGES), "none", or a list ("4.2.4 4.3.0")
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$root/build/tailwind.com}
C=${COSMOCC:-$root/build/cosmocc}
nightly=${RUST_NIGHTLY:-nightly-2026-11-01}
CPUS=${CPUS:-x86_64 aarch64}

log() { echo "build.sh: $*" >&2; }

# shellcheck source=scripts/cosmo.sh
. "$root/scripts/cosmo.sh"
cosmo_check

work=$root/build/ape
rm -rf "$work"
mkdir -p "$work" "$(dirname "$OUT")"
cd "$root"

for cpu in $CPUS; do
    d=$work; [ "$cpu" = aarch64 ] && d=$work/.aarch64
    mkdir -p "$d"
    log "Building the Rust library for $cpu"
    rust_staticlib "$cpu" tailwind tailwind "$d/libtailwind.a" ape-mimalloc
    log "Building QuickJS and mimalloc for $cpu"
    c_objects "$cpu" "$d"
    mimalloc_object "$cpu" "$d/mimalloc.o"
    # One object with one global name, tailwind_main.
    # shellcheck disable=SC2086
    sh "$root/scripts/rust_link.sh" "$C" "$cpu" tailwind_main "$d/tailwind.o" $objects "$d/mimalloc.o" "$d/libtailwind.a"
done

cp "$root/native/tailwind/c_src/main.c" "$work/main.c"
case "$CPUS" in
    "x86_64 aarch64") cc=cosmocc ;;
    *) cc=$CPUS-unknown-cosmo-cc ;;
esac
log "Linking $OUT with $cc"
(cd "$work" && sh "$C/bin/$cc" -O2 -o tailwind.com main.c tailwind.o)

# The packages of the script engine. An APE file is also a ZIP file: the
# program reads the directory tailwindcss/ of its own file (/zip).
if [ "${PACKAGES:-all}" != none ]; then
    command -v zip >/dev/null 2>&1 || { log "The program zip is not there. Install it, or build with PACKAGES=none."; exit 1; }
    package_store "$work/zip/tailwindcss"
    log "Adding the packages of $store_count versions of Tailwind CSS"
    (cd "$work/zip" && zip -q -9 -r -X "$work/tailwind.com" tailwindcss)
fi

cp "$work/tailwind.com" "$OUT"
log "wrote $OUT ($(wc -c < "$OUT") bytes)"
