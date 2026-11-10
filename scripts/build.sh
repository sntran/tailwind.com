#!/bin/sh
# Build tailwind.com: one executable file for Linux, macOS, Windows,
# FreeBSD, NetBSD and OpenBSD, on x86_64 and aarch64.
#
# Usage: sh scripts/build.sh [OUT]          (default: build/tailwind.com)
#
# The program is the Rust crate native/tailwind as a staticlib (the std of
# the target *-unknown-linux-musl), with the calls of c_src/cosmo_rust.c,
# linked with cosmocc into one Actually Portable Executable. This is the
# method of BEAM.com for a library of Rust.
#
# On aarch64, Cosmopolitan reserves the registers x18 and x28. The std of
# Rust for Linux uses them. So the aarch64 library and its std build with
# these registers reserved: a nightly Rust with rust-src, and -Zbuild-std.
#
# Environment:
#   COSMOCC          Directory of an unpacked cosmocc (default build/cosmocc;
#                    this script downloads it when it is not there)
#   COSMOCC_VERSION  cosmocc release to download (default 4.0.2)
#   RUST_NIGHTLY     The nightly Rust for aarch64 (default nightly-2026-11-01)
#   CPUS             The CPUs of the file (default "x86_64 aarch64")
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$root/build/tailwind.com}
COSMOCC_VERSION=${COSMOCC_VERSION:-4.0.2}
COSMOCC_SHA256=${COSMOCC_SHA256:-85b8c37a406d862e656ad4ec14be9f6ce474c1b436b9615e91a55208aced3f44}
C=${COSMOCC:-$root/build/cosmocc}
nightly=${RUST_NIGHTLY:-nightly-2026-11-01}
CPUS=${CPUS:-x86_64 aarch64}

log() { echo "build.sh: $*" >&2; }

if [ ! -x "$C/bin/cosmocc" ]; then
    log "Downloading cosmocc $COSMOCC_VERSION"
    mkdir -p "$C"
    curl -fsSL -o "$C.zip" \
        "https://github.com/jart/cosmopolitan/releases/download/$COSMOCC_VERSION/cosmocc-$COSMOCC_VERSION.zip"
    sum=$(sha256sum "$C.zip" | cut -d' ' -f1)
    if [ "$sum" != "$COSMOCC_SHA256" ]; then
        log "cosmocc-$COSMOCC_VERSION.zip has the SHA-256 $sum, not $COSMOCC_SHA256"
        exit 1
    fi
    (cd "$C" && unzip -q "$C.zip")
    rm -f "$C.zip"
fi

# The calls of the Rust library that c_src/cosmo_rust.c translates.
SYMS="syscall clock_gettime clock_nanosleep __errno_location pthread_attr_init
pthread_attr_destroy pthread_attr_setstacksize pthread_attr_getstack
pthread_attr_getguardsize pthread_getattr_np pthread_create open openat mmap
getauxval sched_getaffinity fstat stat lstat fstatat"

work=$root/build/ape
rm -rf "$work"
mkdir -p "$work" "$(dirname "$OUT")"
cd "$root"

objects=
for cpu in $CPUS; do
    target=$cpu-unknown-linux-musl
    d=$work; [ "$cpu" = aarch64 ] && d=$work/.aarch64
    mkdir -p "$d"
    log "Building the Rust library for $cpu"
    if [ "$cpu" = aarch64 ]; then
        RUSTFLAGS="-C target-feature=+reserve-x18,+reserve-x28" \
            cargo "+$nightly" rustc -q -p tailwind --lib --release --crate-type staticlib \
            --target $target -Zbuild-std=std,panic_unwind --target-dir "$root/target/reserved"
        cp "$root/target/reserved/$target/release/libtailwind.a" "$d/libtailwind.a"
    else
        cargo rustc -q -p tailwind --lib --release --crate-type staticlib --target $target
        cp "$root/target/$target/release/libtailwind.a" "$d/libtailwind.a"
    fi
    args=; for s in $SYMS; do args="$args --redefine-sym $s=tw_rs_$s"; done
    # shellcheck disable=SC2086
    sh "$C/bin/$cpu-unknown-cosmo-objcopy" $args "$d/libtailwind.a"
    sh "$C/bin/$cpu-unknown-cosmo-cc" -O2 -c -o "$d/shim.o" "$root/c_src/cosmo_rust.c"
    # One object with one global name, tailwind_main.
    sh "$root/scripts/rust_link.sh" "$C" "$cpu" tailwind_main "$d/tailwind.o" "$d/shim.o" "$d/libtailwind.a"
    objects="$objects $cpu"
done

cp "$root/c_src/main.c" "$work/main.c"
case "$CPUS" in
    "x86_64 aarch64") cc=cosmocc ;;
    *) cc=$CPUS-unknown-cosmo-cc ;;
esac
log "Linking $OUT with $cc"
(cd "$work" && sh "$C/bin/$cc" -O2 -o tailwind.com main.c tailwind.o)
cp "$work/tailwind.com" "$OUT"
log "wrote $OUT ($(wc -c < "$OUT") bytes)"
