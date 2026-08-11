#!/usr/bin/env bash

set -euo pipefail

readonly gstreamer_version="1.28.5"
readonly runtime_url="https://gstreamer.freedesktop.org/data/pkg/osx/1.28.5/gstreamer-1.0-1.28.5-universal.pkg"
readonly runtime_sha256="0a8fc7a1cf8d7bac833ca0ebe2fd196a199c2465e810cd5b1e4b4f720c258f43"
readonly devel_url="https://gstreamer.freedesktop.org/data/pkg/osx/1.28.5/gstreamer-1.0-devel-1.28.5-universal.pkg"
readonly devel_sha256="6f7b55e8fb86dcc615c9cae46b79b7785851e5c77f79a938648a81dfa2603729"
readonly sdk_root="/Library/Frameworks/GStreamer.framework/Versions/1.0"

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "HTTP Client only packages the macOS arm64 GStreamer runtime; found $(uname -m)" >&2
  exit 1
fi

readonly download_dir="$(mktemp -d "${TMPDIR:-/tmp}/gpui-gstreamer-macos.XXXXXX")"
cleanup() {
  rm -rf "$download_dir"
}
trap cleanup EXIT

install_package() {
  local name="$1"
  local source_url="$2"
  local expected_sha256="$3"
  local package_path="$download_dir/$name.pkg"

  curl --fail --location --retry 3 --output "$package_path" "$source_url"
  local actual_sha256
  actual_sha256="$(shasum -a 256 "$package_path" | awk '{print $1}')"
  if [[ "$actual_sha256" != "$expected_sha256" ]]; then
    echo "GStreamer $name package checksum mismatch: expected $expected_sha256, got $actual_sha256" >&2
    exit 1
  fi

  sudo installer -pkg "$package_path" -target /
}

install_package "runtime" "$runtime_url" "$runtime_sha256"
install_package "devel" "$devel_url" "$devel_sha256"

if [[ ! -d "$sdk_root" ]]; then
  echo "GStreamer $gstreamer_version installers completed but expected SDK root is missing: $sdk_root" >&2
  exit 1
fi

readonly pkg_config="$sdk_root/bin/pkg-config"
readonly gstreamer_pc="$sdk_root/lib/pkgconfig/gstreamer-1.0.pc"
if [[ ! -x "$pkg_config" ]]; then
  echo "GStreamer SDK pkg-config executable is missing: $pkg_config" >&2
  exit 1
fi
if [[ ! -f "$gstreamer_pc" ]]; then
  echo "GStreamer SDK pkg-config metadata is missing: $gstreamer_pc" >&2
  exit 1
fi

export PKG_CONFIG_PATH="$sdk_root/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export DYLD_FALLBACK_LIBRARY_PATH="$sdk_root/lib${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"
export PATH="$sdk_root/bin:$PATH"
export PKG_CONFIG="$pkg_config"
export GPUI_PKG_CONFIG="$pkg_config"
export GPUI_GSTREAMER_SDK_ROOT="$sdk_root"

if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    printf 'PKG_CONFIG_PATH=%s\n' "$PKG_CONFIG_PATH"
    printf 'DYLD_FALLBACK_LIBRARY_PATH=%s\n' "$DYLD_FALLBACK_LIBRARY_PATH"
    printf 'PATH=%s\n' "$PATH"
    printf 'PKG_CONFIG=%s\n' "$pkg_config"
    printf 'GPUI_PKG_CONFIG=%s\n' "$pkg_config"
    printf 'GPUI_GSTREAMER_SDK_ROOT=%s\n' "$sdk_root"
  } >> "$GITHUB_ENV"
fi

gst-inspect-1.0 --version
