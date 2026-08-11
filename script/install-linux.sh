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
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-tools \
  gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad \
  gstreamer1.0-libav \
  gstreamer1.0-pulseaudio \
  pkg-config \
  vulkan-validationlayers \
  libvulkan1

if ! sudo apt-get install -y libayatana-appindicator3-dev; then
  sudo apt-get install -y libappindicator3-dev
fi
