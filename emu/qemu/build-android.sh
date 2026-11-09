#!/usr/bin/env bash
# Cross-build QEMU (TCG + rutabaga/gfxstream) for aarch64 Android, for the emulated engine.
#
# This produces the executable the emulated engine ships as
# `android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so`. It is a PIE named `lib*.so`
# so the Android packager extracts it into the app's nativeLibraryDir, which is the only place an
# untrusted app may execute from (untrusted_app* W^X for app data).
#
# Usage:
#   emu/qemu/build-android.sh --check [--require-pins] [--downloads-dir DIR]
#   emu/qemu/build-android.sh --build [--downloads-dir DIR] [--jobs N] [--output PATH]
#
# --check never touches the network and never needs the NDK; it is what `scripts/verify.sh` runs.
# Nothing here is claimed to have been exercised: the first real build is Gate 2, and its binary
# size, `-device help` output and NDK provenance belong in a qualification report.

set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
qemu_dir="$repo_root/emu/qemu"
versions="$qemu_dir/VERSIONS.toml"
downloads_dir="$qemu_dir/downloads"
output="$repo_root/android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so"
mode=""
require_pins="no"
jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"

usage() {
  sed -n '3,18p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) mode="check" ;;
    --build) mode="build" ;;
    --require-pins) require_pins="yes" ;;
    --downloads-dir) downloads_dir="${2:?--downloads-dir needs a directory}"; shift ;;
    --jobs) jobs="${2:?--jobs needs a number}"; shift ;;
    --output) output="${2:?--output needs a path}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 64 ;;
  esac
  shift
done

if [[ -z "$mode" ]]; then
  usage >&2
  exit 64
fi

if [[ ! -f "$versions" ]]; then
  echo "missing pins file: $versions" >&2
  exit 2
fi

# Prints `<section> <key> <value>` for every string entry, so the shell can verify archives and
# the NDK without a TOML parser of its own.
pins() {
  python3 - "$versions" <<'PY'
import sys, tomllib

with open(sys.argv[1], "rb") as handle:
    data = tomllib.load(handle)
for section, values in data.items():
    if not isinstance(values, dict):
        continue
    for key, value in values.items():
        if isinstance(value, (str, int)):
            print(f"{section}\t{key}\t{value}")
PY
}

pin() {
  local section="$1" key="$2"
  pins | awk -F '\t' -v section="$section" -v key="$key" '$1 == section && $2 == key { print $3 }'
}

check() {
  local version
  version="$(pin qemu version)"
  echo "emulated engine QEMU pins"
  echo "  qemu        $(pin qemu version)   $(pin qemu url)"
  echo "  rutabaga    $(pin rutabaga_gfx version) (must equal Cargo.lock)"
  echo "  gfxstream   $(pin gfxstream revision)"
  echo "  glib        $(pin glib version)"
  echo "  pixman      $(pin pixman version)"
  echo "  libslirp    $(pin libslirp version)"
  echo "  libfdt      $(pin libfdt version)"
  echo "  zlib        $(pin zlib version)"
  echo "  libffi      $(pin libffi version)"
  echo "  pcre2       $(pin pcre2 version)"
  echo "  ndk         $(pin ndk version) (minSdk $(pin ndk min_sdk), target $(pin ndk target))"

  local locked
  locked="$(grep -A1 '^name = "rutabaga_gfx"' "$repo_root/Cargo.lock" | awk -F'"' '/version/ { print $2 }')"
  if [[ -n "$locked" && "$locked" != "$(pin rutabaga_gfx version)" ]]; then
    echo "rutabaga_gfx pin $(pin rutabaga_gfx version) does not match Cargo.lock ($locked)" >&2
    return 1
  fi

  if [[ "$require_pins" == "yes" ]]; then
    local missing=0 section key value
    while IFS=$'\t' read -r section key value; do
      if [[ "$key" == "sha256" && -z "$value" ]]; then
        echo "unpinned source hash: $section.sha256" >&2
        missing=1
      fi
    done < <(pins)
    if [[ "$missing" != 0 ]]; then
      echo "refusing to build from sources with unrecorded hashes (use the pins in VERSIONS.toml)" >&2
      return 1
    fi
  fi

  local checked=0 failed=0 archive recorded actual
  if [[ -d "$downloads_dir" ]]; then
    while IFS= read -r archive; do
      # The archive belongs to the section whose `archive` key, or else whose `url` basename,
      # equals its file name; compare against that section's `sha256`.
      recorded="$(pins | awk -F '\t' -v name="$(basename "$archive")" '
        $2 == "url" { n = split($3, parts, "/"); if (parts[n] == name) owner = $1 }
        $2 == "archive" { if ($3 == name) owner = $1 }
        $2 == "sha256" { hash[$1] = $3 }
        END { if (owner != "") print hash[owner] }')"
      if [[ -z "$recorded" ]]; then
        echo "  present but unrecorded: $(basename "$archive")" >&2
        failed=1
        continue
      fi
      actual="$(shasum -a 256 "$archive" | awk '{ print $1 }')"
      checked=$((checked + 1))
      if [[ "$actual" != "$recorded" ]]; then
        echo "  sha256 mismatch: $(basename "$archive")" >&2
        echo "    recorded $recorded" >&2
        echo "    actual   $actual" >&2
        failed=1
      fi
    done < <(find "$downloads_dir" -maxdepth 1 -type f \( -name '*.tar.*' -o -name '*.zip' \) | sort)
  fi
  echo "  archives verified: $checked (in ${downloads_dir#"$repo_root"/})"
  return "$failed"
}

# Unpacks a pinned archive into a fresh source directory and prints that directory.
unpack() {
  local dep="$1" archive dest
  archive="$(pin "$dep" archive)"
  archive="$downloads_dir/${archive:-$(basename "$(pin "$dep" url)")}"
  if [[ ! -f "$archive" ]]; then
    echo "missing archive: $archive (download $(pin "$dep" url))" >&2
    exit 2
  fi
  dest="$build_dir/src/$dep"
  rm -rf "$dest"
  mkdir -p "$dest"
  tar -xf "$archive" -C "$dest" --strip-components=1
  apply_patches "$dep" "$dest"
  printf '%s\n' "$dest"
}

# Applies emu/qemu/patches/<dep>/*.patch in name order. Every patch carries a header comment
# explaining why it exists; `patch` skips that leading text.
apply_patches() {
  local dep="$1" dest="$2" patch_file
  for patch_file in "$qemu_dir/patches/$dep"/*.patch; do
    [[ -e "$patch_file" ]] || continue
    patch -d "$dest" -p1 --forward --quiet <"$patch_file" >&2 \
      || { echo "patch failed: $patch_file" >&2; exit 1; }
  done
}

# meson cross file with this build's absolute dependency prefix.
write_cross_file() {
  sed -e "s|@DEPS@|$deps_dir|g" "$qemu_dir/cross/aarch64-android.ini" >"$build_dir/cross.ini"
}

meson_dep() {
  local dep="$1"; shift
  local src
  src="$(unpack "$dep")"
  meson setup "$src/_build" "$src" --cross-file "$build_dir/cross.ini" --prefix "$deps_dir" \
    --libdir lib --buildtype release -Ddefault_library=static "$@" >"$build_dir/log/$dep.setup.log" 2>&1 \
    || { tail -40 "$build_dir/log/$dep.setup.log" >&2; exit 1; }
  ninja -C "$src/_build" -j"$jobs" install >"$build_dir/log/$dep.build.log" 2>&1 \
    || { tail -40 "$build_dir/log/$dep.build.log" >&2; exit 1; }
}

autotools_dep() {
  local dep="$1"; shift
  local src
  src="$(unpack "$dep")"
  (cd "$src" && ./configure --host=aarch64-linux-android --prefix="$deps_dir" \
      --enable-static --disable-shared "$@" >"$build_dir/log/$dep.setup.log" 2>&1 \
    && make -j"$jobs" install >"$build_dir/log/$dep.build.log" 2>&1) \
    || { tail -40 "$build_dir/log/$dep."*.log >&2; exit 1; }
}

build_deps() {
  local src
  echo "== zlib $(pin zlib version)"
  src="$(unpack zlib)"
  (cd "$src" && CHOST=aarch64-linux-android ./configure --static --prefix="$deps_dir" \
      >"$build_dir/log/zlib.setup.log" 2>&1 && make -j"$jobs" install >"$build_dir/log/zlib.build.log" 2>&1) \
    || { tail -40 "$build_dir/log/zlib."*.log >&2; exit 1; }

  echo "== libffi $(pin libffi version)"
  autotools_dep libffi --disable-docs --disable-multi-os-directory

  echo "== pcre2 $(pin pcre2 version)"
  autotools_dep pcre2 --disable-pcre2grep-libz --disable-pcre2grep-libbz2 --enable-pcre2-8 \
    --disable-pcre2-16 --disable-pcre2-32

  echo "== glib $(pin glib version)"
  meson_dep glib -Dtests=false -Dselinux=disabled -Dlibmount=disabled -Dxattr=false \
    -Dintrospection=disabled -Dnls=disabled -Dlibelf=disabled -Dsysprof=disabled \
    -Dman-pages=disabled -Ddocumentation=false -Ddtrace=disabled -Dsystemtap=disabled \
    -Dglib_debug=disabled -Dinstalled_tests=false

  echo "== pixman $(pin pixman version)"
  meson_dep pixman -Dtests=disabled -Ddemos=disabled -Dgtk=disabled -Dlibpng=disabled \
    -Dopenmp=disabled

  echo "== libslirp $(pin libslirp version)"
  meson_dep libslirp

  echo "== libfdt (dtc) $(pin libfdt version)"
  meson_dep libfdt -Dtools=false -Dpython=disabled -Dtests=false -Dyaml=disabled -Dvalgrind=disabled

  # dtc installs a shared libfdt regardless of default_library; QEMU must link the archive.
  find "$deps_dir/lib" -name '*.so*' -delete
}

# gfxstream host backend (Vulkan + composer, and the GLES decoder Soong also builds). The guest
# sees only the capsets QEMU enables: gfxstream-vulkan and x-gfxstream-composer, as CVD1 does.
build_gfxstream() {
  echo "== gfxstream $(pin gfxstream revision)"
  meson_dep gfxstream -Ddecoders=gles,vulkan,composer
}

# rutabaga_gfx C FFI, static, from the pinned git tag with the locked crate graph.
build_rutabaga_ffi() {
  echo "== rutabaga_gfx_ffi $(pin rutabaga_gfx version) ($(pin rutabaga_gfx revision))"
  local src cargo_bin min_sdk
  src="$(unpack rutabaga_gfx)"
  cp "$qemu_dir/patches/rutabaga_gfx/Cargo.lock" "$src/Cargo.lock"
  cargo_bin="$(rustup which cargo 2>/dev/null)" \
    || { echo "rustup is required (rutabaga pins its own rust-toolchain.toml)" >&2; exit 2; }
  min_sdk="$(pin ndk min_sdk)"
  # rust-toolchain.toml in the source asks for 1.88.0; the workspace's stable toolchain is used
  # so the build never downloads a toolchain.
  (cd "$src" && env RUSTUP_TOOLCHAIN=stable PATH="$(dirname "$cargo_bin"):$PATH" \
      PKG_CONFIG_ALLOW_CROSS=1 USE_CLANG=1 \
      CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="aarch64-linux-android$min_sdk-clang" \
      CC_aarch64_linux_android="aarch64-linux-android$min_sdk-clang" \
      CXX_aarch64_linux_android="aarch64-linux-android$min_sdk-clang++" \
      AR_aarch64_linux_android=llvm-ar \
      cargo build --locked --release --manifest-path ffi/Cargo.toml --target aarch64-linux-android \
        --features gfxstream --target-dir "$build_dir/rutabaga-target" \
      >"$build_dir/log/rutabaga.build.log" 2>&1) \
    || { tail -40 "$build_dir/log/rutabaga.build.log" >&2; exit 1; }

  cp "$build_dir/rutabaga-target/aarch64-linux-android/release/librutabaga_gfx_ffi.a" "$deps_dir/lib/"
  mkdir -p "$deps_dir/include/rutabaga_gfx"
  cp "$src/ffi/src/include/rutabaga_gfx_ffi.h" "$deps_dir/include/rutabaga_gfx/"
  # Everything static except Android system libraries, so the APK ships one executable.
  cat >"$deps_dir/lib/pkgconfig/rutabaga_gfx_ffi.pc" <<PC
prefix=$deps_dir
includedir=\${prefix}/include
libdir=\${prefix}/lib

Name: rutabaga_gfx_ffi
Description: C FFI bindings to Rutabaga VGI (static, gfxstream, NDK libc++)
Version: $(pin rutabaga_gfx version)
Libs: -L\${libdir} -lrutabaga_gfx_ffi -lgfxstream_backend -l:libc++_static.a -l:libc++abi.a -lnativewindow -llog -ldl -lm
Cflags: -I\${includedir}
PC
}

build_qemu() {
  echo "== QEMU $(pin qemu version)"
  local src="$build_dir/src/qemu" obj="$build_dir/qemu" min_sdk
  src="$(unpack qemu)"
  min_sdk="$(pin ndk min_sdk)"
  rm -rf "$obj"
  mkdir -p "$obj"
  # QEMU's configure writes its own meson cross file from these. Only the features the CVD1
  # machine needs are enabled; --without-default-features turns everything else off.
  (cd "$obj" && AR=llvm-ar NM=llvm-nm STRIP=llvm-strip RANLIB=llvm-ranlib OBJCOPY=llvm-objcopy \
      PKG_CONFIG=pkg-config "$src/configure" \
      --cross-prefix=aarch64-linux-android- \
      --cc="aarch64-linux-android$min_sdk-clang" --cxx="aarch64-linux-android$min_sdk-clang++" \
      --host-cc=/usr/bin/cc --cpu=aarch64 --target-list=aarch64-softmmu \
      --without-default-features --enable-tcg --enable-slirp --enable-fdt=system --enable-pixman \
      --enable-rutabaga-gfx --enable-vhost-user --disable-docs --disable-werror \
      --extra-cflags="-I$deps_dir/include" --extra-ldflags="-L$deps_dir/lib" \
      >"$build_dir/log/qemu.configure.log" 2>&1) \
    || { tail -40 "$build_dir/log/qemu.configure.log" >&2; exit 1; }
  ninja -C "$obj" -j"$jobs" qemu-system-aarch64 >"$build_dir/log/qemu.build.log" 2>&1 \
    || { grep -E 'error|undefined' "$build_dir/log/qemu.build.log" | head -40 >&2; exit 1; }

  llvm-strip -o "$output" "$obj/qemu-system-aarch64"
  {
    echo "qemu-system-aarch64 for aarch64-linux-android$min_sdk"
    echo "sha256 $(shasum -a 256 "$output" | awk '{ print $1 }')"
    echo "bytes  $(wc -c <"$output" | tr -d ' ')"
    echo "ndk    $ndk_revision ($host_tag)"
    echo "built  $(date -u +%Y-%m-%dT%H:%M:%SZ) on $(uname -sm)"
    pins | awk -F '\t' '$2 == "version" || $2 == "revision" || $2 == "sha256" { print "pin    " $1 "." $2 " " $3 }'
  } | tee "$build_dir/provenance.txt"
}

build() {
  local ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
  if [[ -z "$ndk" ]]; then
    echo "set ANDROID_NDK_HOME to the pinned NDK ($(pin ndk version))" >&2
    exit 2
  fi
  ndk_revision="$(awk -F' = ' '$1 == "Pkg.Revision" { print $2 }' "$ndk/source.properties" 2>/dev/null)"
  if [[ "$ndk_revision" != "$(pin ndk version)" ]]; then
    echo "NDK at $ndk is '$ndk_revision', pinned $(pin ndk version)" >&2
    exit 2
  fi
  local toolchain="$ndk/toolchains/llvm/prebuilt"
  local host_tag
  host_tag="$(ls "$toolchain" 2>/dev/null | head -n 1)"
  if [[ -z "$host_tag" ]]; then
    echo "no LLVM toolchain under $toolchain" >&2
    exit 2
  fi
  export PATH="$toolchain/$host_tag/bin:$PATH"
  export CC="aarch64-linux-android$(pin ndk min_sdk)-clang"
  export CXX="${CC}++"
  export AR=llvm-ar
  export RANLIB=llvm-ranlib
  export STRIP=llvm-strip
  export CFLAGS="-fPIC -O2"
  export CXXFLAGS="-fPIC -O2"

  check

  build_dir="$qemu_dir/build-android"
  deps_dir="$build_dir/deps"
  mkdir -p "$deps_dir" "$build_dir/log" "$(dirname "$output")"
  # Only the cross prefix is visible to pkg-config; host libraries must never leak in.
  export PKG_CONFIG_LIBDIR="$deps_dir/lib/pkgconfig:$deps_dir/share/pkgconfig"
  unset PKG_CONFIG_PATH
  write_cross_file
  echo "build directory: $build_dir (NDK $ndk_revision, $host_tag)"

  if [[ "${CARBON_EMU_SKIP_DEPS:-0}" != 1 ]]; then
    build_deps
    build_gfxstream
    build_rutabaga_ffi
  fi
  build_qemu
  echo "packaged: $output"
}

case "$mode" in
  check) check ;;
  build) build ;;
esac
