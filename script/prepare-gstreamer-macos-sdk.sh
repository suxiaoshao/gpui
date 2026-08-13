#!/usr/bin/env bash

set -euo pipefail

readonly gstreamer_version="1.28.6"
readonly devel_url="https://gstreamer.freedesktop.org/data/pkg/osx/1.28.6/gstreamer-1.0-devel-1.28.6-universal.pkg"
readonly devel_sha256="177b1428d0f47b844e7bff2aeeb22047686d802eba21580dab52f4a6fe1dcf02"
readonly components=(
  "base-system-1.0-devel-1.28.6-universal.pkg"
  "base-crypto-devel-1.28.6-universal.pkg"
  "gstreamer-1.0-core-devel-1.28.6-universal.pkg"
  "gstreamer-1.0-system-devel-1.28.6-universal.pkg"
  "gstreamer-1.0-playback-devel-1.28.6-universal.pkg"
  "gstreamer-1.0-codecs-devel-1.28.6-universal.pkg"
  "gstreamer-1.0-libav-devel-1.28.6-universal.pkg"
)
readonly script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly workspace_dir="$(dirname -- "$script_dir")"

package_path=""
runtime_framework="${GPUI_GSTREAMER_RUNTIME_OUTPUT:-$workspace_dir/target/gstreamer-runtime/macos/GStreamer.framework}"
output_sdk="${GPUI_GSTREAMER_SDK_OUTPUT:-$workspace_dir/target/gstreamer-sdk/macos/GStreamer.framework/Versions/1.0}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package)
      package_path="$2"
      shift 2
      ;;
    --runtime)
      runtime_framework="$2"
      shift 2
      ;;
    --output)
      output_sdk="$2"
      shift 2
      ;;
    --help)
      echo "usage: $0 [--package official-devel.pkg] [--runtime GStreamer.framework] [--output sdk-root]"
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! -d "$runtime_framework/Versions/1.0" ]]; then
  "$script_dir/prepare-gstreamer-macos-runtime.sh" --output "$runtime_framework"
fi

readonly work_dir="$(mktemp -d "${TMPDIR:-/tmp}/gpui-gstreamer-sdk.XXXXXX")"
cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT

if [[ -z "$package_path" ]]; then
  package_path="$work_dir/gstreamer-devel.pkg"
  curl --fail --location --retry 3 --output "$package_path" "$devel_url"
fi

actual_sha256="$(shasum -a 256 "$package_path" | awk '{print $1}')"
if [[ "$actual_sha256" != "$devel_sha256" ]]; then
  echo "GStreamer development package checksum mismatch: expected $devel_sha256, got $actual_sha256" >&2
  exit 1
fi

readonly expanded="$work_dir/expanded"
pkgutil --expand-full "$package_path" "$expanded"

rm -rf "$output_sdk"
mkdir -p "$output_sdk"
/usr/bin/ditto "$runtime_framework/Versions/1.0" "$output_sdk"

for component in "${components[@]}"; do
  payload="$expanded/$component/Payload"
  if [[ ! -d "$payload" ]]; then
    echo "GStreamer $gstreamer_version development component payload is missing: $component" >&2
    exit 1
  fi
  /usr/bin/ditto "$payload" "$output_sdk"
done

for required in \
  "lib/pkgconfig/gstreamer-1.0.pc" \
  "lib/pkgconfig/gstreamer-app-1.0.pc" \
  "lib/pkgconfig/gstreamer-video-1.0.pc" \
  "include/gstreamer-1.0/gst/gst.h" \
  "lib/libgstreamer-1.0.0.dylib"; do
  if [[ ! -e "$output_sdk/$required" ]]; then
    echo "prepared GStreamer SDK is missing $required" >&2
    exit 1
  fi
done

marker_directory="$output_sdk/share/http-client-sdk"
mkdir -p "$marker_directory"
printf '%s\n' "$devel_sha256" > "$marker_directory/source-sha256.txt"

echo "prepared GStreamer SDK: $output_sdk"
if [[ -n "${GITHUB_ENV:-}" ]]; then
  printf 'GPUI_GSTREAMER_SDK_ROOT=%s\n' "$output_sdk" >> "$GITHUB_ENV"
fi
