#!/usr/bin/env bash
# Host test for the `-display android` frame protocol (Phase 4) and its buffer ring.
#
# The header under test is not in the repository as a plain file: it ships inside the QEMU patch
# that also carries the backend, because QEMU is not vendored. This script extracts it with
# `git apply --include`, so the test always exercises exactly the bytes the patch delivers, then
# compiles the test with the host compiler (no NDK, no device, no QEMU build).
#
# usage:
#   qualification/emu/frame-proto/run.sh           # verify against golden-messages.txt
#   qualification/emu/frame-proto/run.sh --emit    # rewrite golden-messages.txt (protocol change)
#
# Safe to run in scripts/verify.sh: no network, no Android tooling, exits non-zero on any failure.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"
patch_file="$repo_root/emu/qemu/patches/qemu/0003-ui-add-android-ahb-backend.patch"
golden="$here/golden-messages.txt"

if [[ ! -f "$patch_file" ]]; then
  echo "missing $patch_file (the android display backend patch)" >&2
  exit 2
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

(cd "$work" && git apply --include='*android-proto.h' "$patch_file")
if [[ ! -f "$work/ui/android-proto.h" ]]; then
  echo "the patch no longer delivers ui/android-proto.h" >&2
  exit 2
fi

cc_bin="${CC:-cc}"
"$cc_bin" -std=c11 -O1 -Wall -Wextra -Werror -Wno-unused-parameter \
  -I"$work" -o "$work/test-android-proto" "$here/test-android-proto.c"

if [[ "${1:-}" == "--emit" ]]; then
  CARBON_EMU_EMIT_GOLDEN="$golden" "$work/test-android-proto" "$golden"
  exit 0
fi

"$work/test-android-proto" "$golden"
