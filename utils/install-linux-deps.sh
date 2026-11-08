#!/usr/bin/env bash
# Installs the system libraries that the plugin's window needs to build on Linux: X11, XCB and
# OpenGL, which baseview and egui link to. CI runs it on Ubuntu; on other distributions, install
# the same libraries with their package manager. See Getting Started in docs/DEVELOPMENT.md.
#
# With --screenshots, also what utils/screenshots.sh needs: the headers of ALSA and JACK for
# nih-plug's standalone host, Xvfb with Mesa's software OpenGL, xdotool, ImageMagick and jq. See
# Screenshots in docs/DEVELOPMENT.md.
#
# With --manual, only what utils/build-manual.sh needs to render the user manual as PDF: pandoc,
# LuaLaTeX with the packages of the template, the German hyphenation, and the fonts Latin Modern.
# See User manual in docs/DEVELOPMENT.md.

set -euo pipefail

packages=(
    libgl-dev
    libx11-dev
    libx11-xcb-dev
    libxcb1-dev
    libxcb-dri2-0-dev
    libxcb-icccm4-dev
    libxcursor-dev
    libxkbcommon-dev
)
case "${1:-}" in
"") ;;
--screenshots)
    packages+=(
        imagemagick
        jq
        libasound2-dev
        libgl1-mesa-dri
        libjack-jackd2-dev
        xauth
        xdotool
        xvfb
    )
    ;;
--manual)
    # Not the libraries of the window: the manual needs no build of the plugin.
    packages=(
        fonts-lmodern
        lmodern
        pandoc
        texlive-fonts-recommended
        texlive-lang-german
        texlive-latex-extra
        texlive-latex-recommended
        texlive-luatex
    )
    ;;
*)
    echo "Usage: $0 [--screenshots | --manual]" >&2
    exit 2
    ;;
esac

sudo apt-get update
sudo apt-get install --yes --no-install-recommends "${packages[@]}"
