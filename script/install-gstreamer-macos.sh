#!/usr/bin/env bash

set -euo pipefail

readonly minimum_gstreamer_version="1.20"

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "HTTP Client only packages the macOS arm64 GStreamer runtime; found $(uname -m)" >&2
  exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required to install the macOS GStreamer development SDK" >&2
  exit 1
fi
if ! brew --prefix gstreamer >/dev/null 2>&1; then
  brew install gstreamer
fi
if ! brew --prefix pkgconf >/dev/null 2>&1; then
  brew install pkgconf
fi

readonly sdk_root="$(brew --prefix gstreamer)"
readonly pkg_config="$(brew --prefix pkgconf)/bin/pkg-config"
if ! PKG_CONFIG_PATH="$sdk_root/lib/pkgconfig" \
  "$pkg_config" --atleast-version="$minimum_gstreamer_version" gstreamer-1.0; then
  echo "Homebrew GStreamer must be at least $minimum_gstreamer_version" >&2
  exit 1
fi

readonly gstreamer_pc="$sdk_root/lib/pkgconfig/gstreamer-1.0.pc"
if [[ ! -f "$gstreamer_pc" ]]; then
  echo "GStreamer SDK pkg-config metadata is missing: $gstreamer_pc" >&2
  exit 1
fi

export PKG_CONFIG_PATH="$sdk_root/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export PATH="$sdk_root/bin:$PATH"
export PKG_CONFIG="$pkg_config"
export GPUI_PKG_CONFIG="$pkg_config"
export GPUI_GSTREAMER_SDK_ROOT="$sdk_root"

if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    printf 'PKG_CONFIG_PATH=%s\n' "$PKG_CONFIG_PATH"
    printf 'PATH=%s\n' "$PATH"
    printf 'PKG_CONFIG=%s\n' "$pkg_config"
    printf 'GPUI_PKG_CONFIG=%s\n' "$pkg_config"
    printf 'GPUI_GSTREAMER_SDK_ROOT=%s\n' "$sdk_root"
  } >> "$GITHUB_ENV"
fi

gst-inspect-1.0 --version
