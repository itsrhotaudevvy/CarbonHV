#!/usr/bin/env bash
# Builds the CarbonHV Android APK and reports exactly what was produced.
#
# Release signing is opt-in and never guessed at:
#   CARBONHV_KEYSTORE=/path/to/release.jks CARBONHV_KEYSTORE_PASSWORD=… \
#   CARBONHV_KEY_ALIAS=carbonhv CARBONHV_KEY_PASSWORD=… \
#     scripts/build-apk.sh --release
# Without those variables the release APK is signed with the standard *debug* key, which keeps it
# installable but means it must not be published as a supported release. The output name says
# which case you are in (`-release.apk` vs `-release-debugkey.apk`), and the script prints why.
#
# usage:
#   scripts/build-apk.sh [--release] [--debug] [--out DIR] [--tag vX.Y.Z] [--version-code N]
#   (--release is the default when neither is given)
#
# Requires JDK 17+ and the Android SDK the rest of the repository pins (SDK 36, build-tools 36,
# NDK 28.2.13676358 for the app's native code). Network access is needed only on the first run,
# to resolve Gradle dependencies.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

variant="release"
out_dir="$repo_root/dist"
tag=""

usage() {
  sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) variant="release" ;;
    --debug) variant="debug" ;;
    --out) out_dir="${2:?--out needs a directory}"; shift ;;
    --tag) tag="${2:?--tag needs a release tag}"; shift ;;
    --version-code) export CARBONHV_VERSION_CODE="${2:?--version-code needs a number}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 64 ;;
  esac
  shift
done

if [[ -n "$tag" ]]; then
  export CARBONHV_VERSION="${tag#v}"
  # Monotonic and tag-derived, so an upgrade replaces the previous install.
  if [[ -z "${CARBONHV_VERSION_CODE:-}" && "$CARBONHV_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    export CARBONHV_VERSION_CODE=$(( BASH_REMATCH[1] * 10000 + BASH_REMATCH[2] * 100 + BASH_REMATCH[3] ))
  fi
fi
export CARBONHV_VERSION="${CARBONHV_VERSION:-0.1.0}"
export CARBONHV_VERSION_CODE="${CARBONHV_VERSION_CODE:-1}"

# Same JDK discovery as scripts/verify.sh, so the two agree on what "the toolchain" is.
if [[ -z "${JAVA_HOME:-}" ]]; then
  if command -v brew >/dev/null && [[ -d "$(brew --prefix openjdk@17 2>/dev/null)/libexec/openjdk.jdk/Contents/Home" ]]; then
    export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
  fi
fi
: "${JAVA_HOME:?set JAVA_HOME to JDK 17 or newer, or install openjdk@17}"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$repo_root/.gradle-local}"

task="assembleRelease"
apk_dir="android/app/build/outputs/apk/release"
base_name="app-release.apk"
signed_label="release"
if [[ "$variant" == "debug" ]]; then
  task="assembleDebug"
  apk_dir="android/app/build/outputs/apk/debug"
  base_name="app-debug.apk"
  signed_label="debug"
elif [[ -z "${CARBONHV_KEYSTORE:-}" || -z "${CARBONHV_KEYSTORE_PASSWORD:-}" ]]; then
  signed_label="release-debugkey"
fi

echo "== building :android:app:$task (version $CARBONHV_VERSION, code $CARBONHV_VERSION_CODE)"
./gradlew --no-daemon ":android:app:$task"

built="$apk_dir/$base_name"
if [[ ! -f "$built" ]]; then
  echo "expected $built; the APK was not produced" >&2
  ls -l "$apk_dir" >&2 || true
  exit 1
fi

mkdir -p "$out_dir"
artifact="$out_dir/carbonhv-${CARBONHV_VERSION}-${signed_label}.apk"
cp "$built" "$artifact"

size="$(wc -c <"$artifact" | tr -d ' ')"
digest="$(shasum -a 256 "$artifact" | awk '{ print $1 }')"

if [[ "$signed_label" == "release-debugkey" ]]; then
  cat >&2 <<'NOTE'
warning: this release APK is signed with the debug key because CARBONHV_KEYSTORE and
         CARBONHV_KEYSTORE_PASSWORD are not set. It installs, but it is not a distributable
         build: publish it only as a clearly labelled CI artifact, never as a supported release.
NOTE
fi

echo "artifact $artifact"
echo "bytes    $size"
echo "sha256   $digest"
echo "signing  $signed_label"
