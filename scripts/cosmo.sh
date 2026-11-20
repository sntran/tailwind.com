# shellcheck shell=sh
# The steps of a build with Cosmopolitan, for scripts/build.sh (the file
# tailwind.com) and scripts/nif.sh (the native NIF objects for BEAM.com).
# These scripts read this file with ".", after they set:
#   root     The directory of the repository
#   C        The directory of an unpacked cosmocc
#   nightly  The nightly Rust for aarch64
#   log      A function that writes a message

# The cosmocc of sntran/cosmopolitan: the official cosmocc 4.0.2, with
# the libc, the tools and the loaders of the patches of BEAM.com (its
# PATCHES.md). These patches give the aarch64 libc a Windows layer and
# the loader bin/ape-arm64.exe: the file then runs as native code on
# Windows on ARM64 (native/tailwind/c_src/main.c). They also give a
# loader for macOS arm64 that checks its fixed addresses (C51 and C57 in
# docs/UPSTREAM.md of BEAM.com).
COSMOCC_VERSION=${COSMOCC_VERSION:-4.0.2-patched.1}
COSMOCC_SHA256=${COSMOCC_SHA256:-9393ba3beba565c6007edadfd29cfaa16f31e80ad9f1d65b7cbb6ad0740b6bec}
COSMOCC_URL=${COSMOCC_URL:-https://github.com/sntran/cosmopolitan/releases/download/$COSMOCC_VERSION/cosmocc-$COSMOCC_VERSION.zip}

# The calls of the Rust library that native/tailwind/c_src/cosmo_rust.c
# translates.
SYMS="syscall clock_gettime clock_nanosleep __errno_location pthread_attr_init
pthread_attr_destroy pthread_attr_setstacksize pthread_attr_getstack
pthread_attr_getguardsize pthread_getattr_np pthread_create open openat mmap
getauxval sched_getaffinity fstat stat lstat fstatat __xpg_strerror_r"

# cosmo_check: QuickJS is there, and cosmocc is in C (else download it).
# The file VERSION in C has the version of the download. When COSMOCC is
# not set, a cosmocc of another version in C goes away first.
cosmo_check() {
    if [ ! -f "$root/native/tailwind/c_src/quickjs/quickjs.c" ]; then
        log "native/tailwind/c_src/quickjs is empty: run git submodule update --init"
        exit 1
    fi
    if [ -z "${COSMOCC:-}" ] && [ -e "$C" ] && [ "$(cat "$C/VERSION" 2>/dev/null)" != "$COSMOCC_VERSION" ]; then
        log "$C is not cosmocc $COSMOCC_VERSION: removing it"
        rm -rf "$C"
    fi
    if [ ! -x "$C/bin/cosmocc" ]; then
        log "Downloading cosmocc $COSMOCC_VERSION"
        mkdir -p "$C"
        curl -fsSL --retry 3 -o "$C.zip" "$COSMOCC_URL"
        sum=$(sha256sum "$C.zip" | cut -d' ' -f1)
        if [ "$sum" != "$COSMOCC_SHA256" ]; then
            log "cosmocc-$COSMOCC_VERSION.zip has the SHA-256 $sum, not $COSMOCC_SHA256"
            exit 1
        fi
        (cd "$C" && unzip -q "$C.zip")
        rm -f "$C.zip"
        echo "$COSMOCC_VERSION" > "$C/VERSION"
    fi
}

# rust_staticlib CPU PACKAGE LIB A [FEATURES]: the library LIB of the
# crate PACKAGE, with FEATURES (default: `ape`) and no default feature (the crate `mimalloc`
# compiles its C with the C compiler of the system), as the staticlib A for CPU (the std of
# *-unknown-linux-musl), in CARGO_TARGET_DIR (default: target). The calls of SYMS get the names of cosmo_rust.c.
#
# On aarch64, Cosmopolitan reserves the registers x18 and x28. The std of
# Rust for Linux uses them. So the aarch64 library and its std build with
# these registers reserved: a nightly Rust with rust-src, and -Zbuild-std.
rust_staticlib() {
    cpu=$1 package=$2 lib=$3 a=$4 features=${5:-ape}
    target=$cpu-unknown-linux-musl
    dir=${CARGO_TARGET_DIR:-$root/target}
    if [ "$cpu" = aarch64 ]; then
        RUSTFLAGS="-C target-feature=+reserve-x18,+reserve-x28" \
            cargo "+$nightly" rustc -q -p "$package" --lib --release --no-default-features --features "$features" --crate-type staticlib \
            --target "$target" -Zbuild-std=std,panic_unwind --target-dir "$dir/reserved"
        cp "$dir/reserved/$target/release/lib$lib.a" "$a"
    else
        cargo rustc -q -p "$package" --lib --release --no-default-features --features "$features" --crate-type staticlib --target "$target" \
            --target-dir "$dir"
        cp "$dir/$target/release/lib$lib.a" "$a"
    fi
    args=
    for s in $SYMS; do args="$args --redefine-sym $s=tw_rs_$s"; done
    # shellcheck disable=SC2086
    sh "$C/bin/$cpu-unknown-cosmo-objcopy" $args "$a"
}

# package_store DIR: the store of the packages of the script engine in
# DIR: the npm packages `tailwindcss` of the variable PACKAGES ("all", the
# default: each version of the file PACKAGES; or a list), the newest
# version in full, and each older version as a patch: the differences
# from the next newer version (tools/twpatch). The tool reads each
# version back and compares it with its package. The number of the
# versions goes into the variable store_count.
package_store() {
    packages=$root/build/packages
    case "${PACKAGES:-all}" in
        all) versions=$(grep -v '^#' "$root/PACKAGES" | cut -d' ' -f1) ;;
        *) versions=$PACKAGES ;;
    esac
    # shellcheck disable=SC2086
    sh "$root/scripts/packages.sh" "$packages" $versions
    # shellcheck disable=SC2086
    versions=$(echo $versions | tr ' ' '\n' | sort -t. -k1,1n -k2,2n -k3,3n | tr '\n' ' ')
    env -u CARGO_TARGET_DIR cargo build -q --release --manifest-path "$root/tools/twpatch/Cargo.toml"
    rm -rf "$1"
    # shellcheck disable=SC2086
    "$root/tools/twpatch/target/release/twpatch" pack "$packages" "$1" $versions
    # shellcheck disable=SC2086
    store_count=$(echo $versions | wc -w | tr -d ' ')
}

# mimalloc_object CPU O: mimalloc 2 for CPU, compiled with cosmocc, as the
# object O (the feature `ape-mimalloc`, src/cosmo_allocator.rs). The C
# source is the one of the crate libmimalloc-sys of Cargo.lock.
mimalloc_object() {
    cpu=$1 o=$2
    manifest=$(cargo metadata -q --format-version 1 --manifest-path "$root/Cargo.toml" |
        grep -o '"manifest_path":"[^"]*/libmimalloc-sys-[^"]*"' | head -n 1 | sed 's/^"manifest_path":"//; s/"$//')
    [ -n "$manifest" ] || { log "Cargo.lock has no crate libmimalloc-sys"; exit 1; }
    mi=$(dirname "$manifest")/c_src/mimalloc/v2
    # MI_USE_BUILTIN_THREAD_POINTER=0: the id of a thread is the address of
    # a thread-local variable. The thread pointer of the CPU (%fs:0 on
    # x86_64, tpidr_el0 on aarch64) is not the thread data of Cosmopolitan,
    # which has its own on each system (x28 on aarch64).
    # MI_PRIM_HAS_PROCESS_ATTACH=1: no constructor and no destructor of
    # the process. mimalloc starts at its first allocation. At the exit of
    # the process, its destructor deleted the pthread key of its threads
    # while the threads of the scanner still freed memory, and Cosmopolitan
    # stopped on an assertion in pthread_getspecific(). The system frees
    # the memory of the process.
    sh "$C/bin/$cpu-unknown-cosmo-cc" -O2 -DNDEBUG -D_GNU_SOURCE -DMI_USE_BUILTIN_THREAD_POINTER=0 \
        -DMI_PRIM_HAS_PROCESS_ATTACH=1 \
        -I"$mi/include" -c -o "$o" "$mi/src/static.c"
}

# c_objects CPU DIR [WATCH_FLAGS [CFLAGS]]: the C files of the program for
# CPU, in DIR: cosmo_rust.c, the file events of the watch (watch.c, with
# WATCH_FLAGS), and the JavaScript engine (QuickJS and its API for Rust).
# CFLAGS go to each file. The C library is Cosmopolitan, so cosmocc
# compiles them: with the feature `ape`, native/tailwind/build.rs
# compiles no C. The list of the objects goes into the variable objects.
c_objects() {
    cpu=$1 d=$2 watch_flags=${3:-} cflags=${4:-}
    src=$root/native/tailwind/c_src
    cc=$C/bin/$cpu-unknown-cosmo-cc
    mkdir -p "$d"
    # shellcheck disable=SC2086
    sh "$cc" -O2 $cflags -c -o "$d/shim.o" "$src/cosmo_rust.c"
    # shellcheck disable=SC2086
    sh "$cc" -O2 $cflags $watch_flags -c -o "$d/watch.o" "$src/watch.c"
    objects="$d/shim.o $d/watch.o"
    for f in quickjs/quickjs quickjs/libregexp quickjs/libunicode quickjs/cutils quickjs/dtoa js; do
        o=$d/js_$(basename "$f").o
        # shellcheck disable=SC2086
        sh "$cc" -O2 $cflags -fwrapv -funsigned-char -D_GNU_SOURCE \
            "-DCONFIG_VERSION=\"$(cat "$src/quickjs/VERSION")\"" \
            -c -o "$o" "$src/$f.c" &
        objects="$objects $o"
    done
    wait
    for o in $objects; do
        [ -f "$o" ] || { log "The build of $o failed"; exit 1; }
    done
}
