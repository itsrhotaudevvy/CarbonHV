#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Ubuntu Touch CVD1 image assembly requires Linux tools (mkfs.ext4, tar)." >&2
  exit 2
fi

: "${UT_ROOTFS_TARBALL:?set UT_ROOTFS_TARBALL to a pinned Ubuntu Touch rootfs tarball}"
: "${UT_ROOTFS_SHA256:?set UT_ROOTFS_SHA256 to the expected SHA-256}"
: "${UT_KERNEL_IMAGE:?set UT_KERNEL_IMAGE to the ARM64 CVD1 kernel Image}"
: "${UT_INITRD_IMAGE:?set UT_INITRD_IMAGE to the CVD1 initramfs}"
: "${UT_KERNEL_CONFIG:?set UT_KERNEL_CONFIG to the .config UT_KERNEL_IMAGE was built from}"
CARBONCTL="${CARBONCTL:-carbonctl}"

for tool in sha256sum tar mkfs.ext4 truncate; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 2; }
done

actual_sha="$(sha256sum "$UT_ROOTFS_TARBALL" | awk '{print $1}')"
if [[ "$actual_sha" != "$UT_ROOTFS_SHA256" ]]; then
  echo "Ubuntu Touch rootfs SHA-256 mismatch" >&2
  exit 3
fi

# A kernel without virtio-gpu/sync_file breaks the CVD1 contract (exit 3); one without virtual
# cpufreq, THP or balloon page reporting boots but cannot run near-native (exit 5). Both refuse.
if ! "$CARBONCTL" guest kernel-config ubuntu-touch "$UT_KERNEL_CONFIG"; then
  echo "UT_KERNEL_CONFIG does not meet CVD1; merge guest/common/kernel/cvd1-near-native.fragment and guest/ubuntu-touch/device/kernel.fragment" >&2
  exit 3
fi

OUTPUT="${CARBON_UT_OUTPUT:-$PWD/out/carbonhv/ubuntu-touch}"
SIZE_MIB="${CARBON_UT_ROOTFS_MIB:-8192}"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$OUTPUT" "$STAGE/rootfs"
tar -xf "$UT_ROOTFS_TARBALL" -C "$STAGE/rootfs"
mkdir -p "$STAGE/rootfs/etc/carbonhv"
cp "$PWD/guest/ubuntu-touch/device/cvd1.toml" "$STAGE/rootfs/etc/carbonhv/device.toml"

truncate -s "${SIZE_MIB}M" "$OUTPUT/rootfs.img"
mkfs.ext4 -q -F -d "$STAGE/rootfs" "$OUTPUT/rootfs.img"
cp "$UT_KERNEL_IMAGE" "$OUTPUT/kernel"
cp "$UT_INITRD_IMAGE" "$OUTPUT/initrd"
cp "$UT_KERNEL_CONFIG" "$OUTPUT/kernel.config"
printf '%s\n' "$actual_sha" > "$OUTPUT/upstream-rootfs.sha256"

echo "Ubuntu Touch CVD1 artifacts staged in $OUTPUT"
