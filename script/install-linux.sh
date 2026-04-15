#!/usr/bin/env bash

set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  build-essential \
  clang \
  gcc \
  g++ \
  libfontconfig1-dev \
  libgtk-3-dev \
  libssl-dev \
  libwayland-dev \
  libwebkit2gtk-4.1-dev \
  libx11-xcb-dev \
  libxdo-dev \
  libxkbcommon-x11-dev \
  libzstd-dev \
  pkg-config \
  vulkan-validationlayers \
  libvulkan1

if ! sudo apt-get install -y libayatana-appindicator3-dev; then
  sudo apt-get install -y libappindicator3-dev
fi
