#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: prepare-controlled-host.sh --android-root DIR --product-makefile PATH \
  (--ui-signing-sha256 HEX | --ui-apk APK [--apksigner PATH])

Stages this CarbonHV checkout at external/carbonhv inside an Android source tree
and wires host/product/carbonhv.mk into the selected product makefile.
USAGE
}

fail() {
  echo "prepare-controlled-host: $*" >&2
  exit 1
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
android_root=""
product_makefile=""
ui_digest=""
ui_apk=""
apksigner_bin=""

while (($#)); do
  case "$1" in
    --android-root)
      (($# >= 2)) || fail '--android-root requires a value'
      android_root="$2"
      shift 2
      ;;
    --product-makefile)
      (($# >= 2)) || fail '--product-makefile requires a value'
      product_makefile="$2"
      shift 2
      ;;
    --ui-signing-sha256)
      (($# >= 2)) || fail '--ui-signing-sha256 requires a value'
      ui_digest="$2"
      shift 2
      ;;
    --ui-apk)
      (($# >= 2)) || fail '--ui-apk requires a value'
      ui_apk="$2"
      shift 2
      ;;
    --apksigner)
      (($# >= 2)) || fail '--apksigner requires a value'
      apksigner_bin="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown argument: $1"
      ;;
  esac
done

[[ -n "$android_root" ]] || fail '--android-root is required'
[[ -n "$product_makefile" ]] || fail '--product-makefile is required'
[[ -d "$android_root" ]] || fail "Android source root does not exist: $android_root"
android_root="$(cd "$android_root" && pwd -P)"
[[ -f "$android_root/build/envsetup.sh" ]] || fail "not an Android source tree: $android_root/build/envsetup.sh is missing"

if [[ "$product_makefile" = /* ]]; then
  product_path="$product_makefile"
else
  product_path="$android_root/$product_makefile"
fi
[[ -f "$product_path" ]] || fail "product makefile does not exist: $product_path"

if [[ -n "$ui_digest" && -n "$ui_apk" ]]; then
  fail 'choose either --ui-signing-sha256 or --ui-apk, not both'
fi

if [[ -z "$ui_digest" ]]; then
  [[ -n "$ui_apk" ]] || fail 'one of --ui-signing-sha256 or --ui-apk is required'
  [[ -f "$ui_apk" ]] || fail "UI APK does not exist: $ui_apk"

  if [[ -z "$apksigner_bin" ]]; then
    if command -v apksigner >/dev/null 2>&1; then
      apksigner_bin="$(command -v apksigner)"
    elif [[ -n "${ANDROID_HOME:-}" && -d "$ANDROID_HOME/build-tools" ]]; then
      apksigner_bin="$(find "$ANDROID_HOME/build-tools" -maxdepth 2 -type f -name apksigner -print | sort -V | tail -n 1)"
    fi
  fi
  [[ -n "$apksigner_bin" && -x "$apksigner_bin" ]] || fail 'apksigner was not found; pass --apksigner or --ui-signing-sha256'

  ui_digest="$({ "$apksigner_bin" verify --print-certs "$ui_apk"; } | awk -F': ' '/certificate SHA-256 digest:/ { print $2; exit }')"
  [[ -n "$ui_digest" ]] || fail 'apksigner did not report a signer certificate SHA-256 digest'
fi

ui_digest="$(printf '%s' "$ui_digest" | tr '[:upper:]' '[:lower:]' | tr -d ':[:space:]')"
[[ "$ui_digest" =~ ^[0-9a-f]{64}$ ]] || fail 'UI signing digest must be exactly 64 hexadecimal characters'

stage_root="$android_root/external/carbonhv"
marker="$stage_root/.carbonhv-controlled-host-staging"
if [[ -d "$stage_root" && ! -f "$marker" ]] && find "$stage_root" -mindepth 1 -maxdepth 1 -print -quit | grep -q .; then
  fail "refusing to overwrite non-CarbonHV staging directory: $stage_root"
fi
mkdir -p "$stage_root"

if [[ "$stage_root" == "$repo_root" || "$stage_root" == "$repo_root/"* ]]; then
  fail 'Android source tree must not stage CarbonHV inside the CarbonHV source checkout'
fi

command -v rsync >/dev/null 2>&1 || fail 'rsync is required to stage the CarbonHV source tree'
rsync -a --delete \
  --exclude '.git/' \
  --exclude '.gradle/' \
  --exclude '.gradle-local/' \
  --exclude '.idea/' \
  --exclude '.kotlin/' \
  --exclude '.superpowers/' \
  --exclude '.worktrees/' \
  --exclude 'build/' \
  --exclude 'target/' \
  --exclude 'captures/' \
  --exclude 'dist/' \
  --exclude 'local.properties' \
  --exclude '.DS_Store' \
  "$repo_root/" "$stage_root/"
printf '%s\n' 'managed by host/product/prepare-controlled-host.sh' > "$marker"

framework_bp="$android_root/packages/modules/Virtualization/libs/framework-virtualization/Android.bp"
[[ -f "$framework_bp" ]] || fail \
  "Android 17 framework-virtualization Android.bp is missing: $framework_bp"

python3 - "$framework_bp" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
carbonhv_visibility = '        "//external/carbonhv:__subpackages__",'
upstream_visibility = '        "//packages/modules/Virtualization:__subpackages__",'

count = text.count(carbonhv_visibility)
if count > 1:
    raise SystemExit(f"duplicate CarbonHV framework visibility entries in {path}")
if count == 0:
    if upstream_visibility not in text:
        raise SystemExit(
            f"expected Android 17 framework-virtualization impl visibility was not found in {path}"
        )
    text = text.replace(
        upstream_visibility,
        upstream_visibility + "\n" + carbonhv_visibility,
        1,
    )
    path.write_text(text)
PY

python3 - "$product_path" "$ui_digest" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
digest = sys.argv[2]
start = "# BEGIN CarbonHV controlled-host integration"
end = "# END CarbonHV controlled-host integration"
text = path.read_text()

if text.count(start) > 1 or text.count(end) > 1:
    raise SystemExit(f"ambiguous CarbonHV integration markers in {path}")
if (start in text) != (end in text):
    raise SystemExit(f"incomplete CarbonHV integration markers in {path}")

block = "\n".join([
    start,
    f"CARBONHV_UI_SIGNING_SHA256 := {digest}",
    "$(call inherit-product, external/carbonhv/host/product/carbonhv.mk)",
    end,
])

if start in text:
    begin = text.index(start)
    finish = text.index(end, begin) + len(end)
    text = text[:begin] + block + text[finish:]
else:
    text = text.rstrip() + "\n\n" + block + "\n"

path.write_text(text)
PY

printf 'staged CarbonHV: %s\n' "$stage_root"
printf 'extended framework-virtualization impl visibility: %s\n' "$framework_bp"
printf 'wired product: %s\n' "$product_path"
printf 'UI signing SHA-256: %s\n' "$ui_digest"
