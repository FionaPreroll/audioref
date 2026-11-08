#!/usr/bin/env bash
# Installs the system libraries that the plugin's window needs to build on Linux: X11, XCB and
# OpenGL, which baseview and egui link to. CI runs it on Ubuntu; on other distributions, install
# the same libraries with their package manager. See Getting Started in docs/DEVELOPMENT.md.

set -euo pipefail

sudo apt-get update
sudo apt-get install --yes --no-install-recommends \
    libgl-dev \
    libx11-dev \
    libx11-xcb-dev \
    libxcb1-dev \
    libxcb-dri2-0-dev \
    libxcb-icccm4-dev \
    libxcursor-dev \
    libxkbcommon-dev
