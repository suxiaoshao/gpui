#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-bundle >/dev/null 2>&1; then
  echo "cargo-bundle 未安装，请先执行: cargo install cargo-bundle" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
WORKSPACE_CARGO_TOML="$(cargo locate-project --workspace --message-format plain)"
WORKSPACE_DIR="$(dirname "${WORKSPACE_CARGO_TOML}")"
BUNDLE_DIR="${WORKSPACE_DIR}/target/release/bundle"

prepare_bundle_icons() {
  local src_png="${APP_DIR}/assets/icon/ChatGPT.icon/Assets/logo.png"
  if [ ! -f "${src_png}" ]; then
    src_png="${APP_DIR}/assets/icon/app-icon.png"
  fi
  local iconset_dir="${APP_DIR}/assets/icon/app-icon.iconset"
  local required_icon="${iconset_dir}/icon_512x512@2x.png"
  local should_regenerate=false

  if [ -f "${required_icon}" ]; then
    if file "${required_icon}" | grep -q "16-bit/color RGBA"; then
      should_regenerate=true
    else
      return 0
    fi
  fi

  if [ ! -f "${src_png}" ]; then
    echo "未找到源图标，跳过 iconset 生成: ${src_png}"
    return 0
  fi

  if ! command -v sips >/dev/null 2>&1; then
    echo "未找到 sips，无法生成 iconset；cargo-bundle 可能无法匹配图标类型"
    return 0
  fi

  mkdir -p "${iconset_dir}"
  if [ "${should_regenerate}" = true ]; then
    rm -f "${iconset_dir}"/*.png
  fi
  for size in 16 32 128 256 512; do
    sips -z "${size}" "${size}" "${src_png}" --out "${iconset_dir}/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "${src_png}" --out "${iconset_dir}/icon_${size}x${size}@2x.png" >/dev/null
  done
}

inject_liquid_glass_icon() {
  local app_path="$1"
  local icon_dir="${APP_DIR}/assets/icon/ChatGPT.icon"
  local plist="${app_path}/Contents/Info.plist"

  if [ ! -d "${icon_dir}" ]; then
    echo "未找到 .icon 目录，跳过 Liquid Glass 图标注入: ${icon_dir}"
    return 0
  fi

  if ! command -v xcrun >/dev/null 2>&1; then
    echo "未找到 xcrun，跳过 Liquid Glass 图标注入（保留普通图标）"
    return 0
  fi

  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir}"' RETURN

  # Build Assets.car from .icon catalog for macOS Liquid Glass icon pipeline.
  if ! xcrun actool "${icon_dir}" \
    --compile "${tmp_dir}" \
    --output-format human-readable-text \
    --notices --warnings --errors \
    --output-partial-info-plist "${tmp_dir}/assetcatalog_generated_info.plist" \
    --app-icon Icon \
    --include-all-app-icons \
    --enable-on-demand-resources NO \
    --development-region en \
    --target-device mac \
    --platform macosx \
    --minimum-deployment-target 26.0; then
    echo "actool 编译失败，跳过 Liquid Glass 图标注入（保留普通图标）"
    return 0
  fi

  if [ ! -f "${tmp_dir}/Assets.car" ]; then
    echo "未生成 Assets.car，跳过 Liquid Glass 图标注入（保留普通图标）"
    return 0
  fi

  cp "${tmp_dir}/Assets.car" "${app_path}/Contents/Resources/Assets.car"

  if [ -x /usr/libexec/PlistBuddy ]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleIconName Icon" "${plist}" >/dev/null 2>&1 \
      || /usr/libexec/PlistBuddy -c "Add :CFBundleIconName string Icon" "${plist}" >/dev/null
  else
    plutil -replace CFBundleIconName -string Icon "${plist}"
  fi

  if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "${app_path}" >/dev/null
  fi

  echo "已注入 Liquid Glass 图标: ${app_path}"
}

cd "${APP_DIR}"
prepare_bundle_icons
cargo bundle --release

if [ "$(uname -s)" = "Darwin" ]; then
  APP_PATH="$(find "${BUNDLE_DIR}/osx" -maxdepth 1 -type d -name "*.app" | head -n 1 || true)"
  if [ -n "${APP_PATH}" ]; then
    inject_liquid_glass_icon "${APP_PATH}"
  else
    echo "未找到 .app 包，跳过 Liquid Glass 图标注入"
  fi
fi

echo "打包完成，产物目录: ${BUNDLE_DIR}/"
