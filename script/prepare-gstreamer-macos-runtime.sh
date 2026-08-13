#!/usr/bin/env bash

set -euo pipefail

readonly gstreamer_version="1.28.6"
readonly runtime_url="https://gstreamer.freedesktop.org/data/pkg/osx/1.28.6/gstreamer-1.0-1.28.6-universal.pkg"
readonly runtime_sha256="a8eb366c59b7e9e5dc049848fed6bcd203a8878aa7517c051639fda78797c6ad"
readonly components=(
  "osx-framework-1.28.6-universal.pkg"
  "base-system-1.0-1.28.6-universal.pkg"
  "base-crypto-1.28.6-universal.pkg"
  "gstreamer-1.0-core-1.28.6-universal.pkg"
  "gstreamer-1.0-system-1.28.6-universal.pkg"
  "gstreamer-1.0-playback-1.28.6-universal.pkg"
  "gstreamer-1.0-codecs-1.28.6-universal.pkg"
  "gstreamer-1.0-libav-1.28.6-universal.pkg"
)
readonly script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly workspace_dir="$(dirname -- "$script_dir")"

package_path=""
output_framework="${GPUI_GSTREAMER_RUNTIME_OUTPUT:-$workspace_dir/target/gstreamer-runtime/macos/GStreamer.framework}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package)
      package_path="$2"
      shift 2
      ;;
    --output)
      output_framework="$2"
      shift 2
      ;;
    --help)
      echo "usage: $0 [--package official-runtime.pkg] [--output GStreamer.framework]"
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

readonly work_dir="$(mktemp -d "${TMPDIR:-/tmp}/gpui-gstreamer-runtime.XXXXXX")"
cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT

if [[ -z "$package_path" ]]; then
  package_path="$work_dir/gstreamer-runtime.pkg"
  curl --fail --location --retry 3 --output "$package_path" "$runtime_url"
fi

actual_sha256="$(shasum -a 256 "$package_path" | awk '{print $1}')"
if [[ "$actual_sha256" != "$runtime_sha256" ]]; then
  echo "GStreamer runtime checksum mismatch: expected $runtime_sha256, got $actual_sha256" >&2
  exit 1
fi

readonly expanded="$work_dir/expanded"
pkgutil --expand-full "$package_path" "$expanded"

rm -rf "$output_framework"
mkdir -p "$(dirname "$output_framework")"

for component in "${components[@]}"; do
  component_root="$expanded/$component"
  payload="$component_root/Payload"
  if [[ ! -d "$payload" ]]; then
    echo "GStreamer $gstreamer_version component payload is missing: $component" >&2
    exit 1
  fi

  if [[ "$component" == osx-framework-* ]]; then
    /usr/bin/ditto "$payload" "$output_framework"
  else
    mkdir -p "$output_framework/Versions/1.0"
    /usr/bin/ditto "$payload" "$output_framework/Versions/1.0"
  fi
done

for required in \
  "Versions/1.0/bin/gst-inspect-1.0" \
  "Versions/1.0/lib/libgstreamer-1.0.0.dylib" \
  "Versions/1.0/lib/gstreamer-1.0" \
  "Versions/1.0/libexec/gstreamer-1.0/gst-plugin-scanner"; do
  if [[ ! -e "$output_framework/$required" ]]; then
    echo "prepared GStreamer framework is missing $required" >&2
    exit 1
  fi
done

marker_directory="$output_framework/Versions/1.0/share/http-client-runtime"
mkdir -p "$marker_directory"
printf '%s\n' "$runtime_sha256" > "$marker_directory/source-sha256.txt"

echo "prepared GStreamer runtime: $output_framework"
if [[ -n "${GITHUB_ENV:-}" ]]; then
  printf 'GPUI_GSTREAMER_RUNTIME_DIR=%s\n' "$output_framework" >> "$GITHUB_ENV"
fi
