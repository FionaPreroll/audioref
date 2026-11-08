#!/usr/bin/env bash
# Takes screenshots of the plugin's window, without a DAW and without a graphics card. AudioRef
# runs as a standalone program (screenshots/) on Xvfb, drawn by Mesa's software OpenGL; xdotool
# loads a small synthetic database in OPRA's format (screenshots/demo_database.jsonl) and picks a
# preset, and ImageMagick's import takes the pictures, in English and in German.
#
# Usage: utils/screenshots.sh [directory], by default target/screenshots.
# Needs the libraries and tools from: utils/install-linux-deps.sh --screenshots
# See Screenshots in docs/DEVELOPMENT.md.

set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
out=$(realpath -m "${1:-$root/target/screenshots}")

# Where the window's widgets are, in its logical pixels, for xdotool. They follow the layout in
# crates/audioref/src/editor.rs: move them when it changes, the pictures show where the clicks
# went.
PATH_FIELD=(290 58)
LOAD_BUTTON=(600 58)
FIRST_HEADPHONES=(60 135)
FIRST_PRESET=(440 153)

# Outside Xvfb: prepare the copy of nih-plug, build the program and run this script again on Xvfb.
if [[ "${AUDIOREF_SCREENSHOTS_XVFB:-}" != 1 ]]; then
    # nih-plug's window asks for a framebuffer with sRGB, which Mesa's software rendering doesn't
    # offer under Xvfb. The program builds with a copy of nih-plug, at the commit AudioRef pins,
    # that asks for one without; egui turns the sRGB conversion off anyway, so nothing else
    # changes. screenshots/Cargo.toml patches nih-plug with the copy.
    rev_of() { sed -n 's/^nih_plug = {.*rev = "\([0-9a-f]*\)".*/\1/p' "$1"; }
    rev=$(rev_of "$root/Cargo.toml")
    if [[ -z "$rev" || "$rev" != "$(rev_of "$root/screenshots/Cargo.toml")" ]]; then
        echo "screenshots/Cargo.toml must pin nih_plug to the same rev as Cargo.toml ($rev)." >&2
        exit 1
    fi
    copy="$root/screenshots/target/nih-plug"
    if [[ "$(cat "$copy/.audioref-rev" 2>/dev/null)" != "$rev" ]]; then
        echo "Copying nih-plug $rev to $copy..."
        source=$(cargo metadata --manifest-path "$root/Cargo.toml" --format-version 1 --locked \
            | jq -r '.packages[] | select(.name == "nih_plug") | .manifest_path')
        rm -rf "$copy"
        mkdir -p "$(dirname "$copy")"
        cp -r "$(dirname "$source")" "$copy"
        editor="$copy/nih_plug_egui/src/editor.rs"
        if ! grep -q 'srgb: true,' "$editor"; then
            echo "$editor no longer asks for sRGB as expected: update utils/screenshots.sh." >&2
            exit 1
        fi
        sed -i 's/srgb: true,/srgb: false,/' "$editor"
        echo "$rev" >"$copy/.audioref-rev"
    fi
    echo "Building the standalone program..."
    # Not --locked: when the plugin's dependencies change, Cargo updates screenshots/Cargo.lock to
    # match, a change to commit.
    cargo build --manifest-path "$root/screenshots/Cargo.toml"
    mkdir -p "$out"
    AUDIOREF_SCREENSHOTS_XVFB=1 exec xvfb-run --auto-servernum \
        --server-args="-screen 0 1280x1024x24 -nolisten tcp" "$0" "$out"
fi

program="$root/screenshots/target/debug/audioref-screenshots"
# The window shows the path: a short one, the same on every computer.
database=/tmp/audioref-screenshots/database_v1.jsonl
mkdir -p "$(dirname "$database")"
cp "$root/screenshots/demo_database.jsonl" "$database"
pid=
trap '[[ -n "$pid" ]] && kill "$pid" 2>/dev/null; true' EXIT

# Clicks at a point of the window and waits for egui to draw the result.
click() {
    xdotool mousemove --window "$window" "$1" "$2" click 1
    sleep 0.5
}

# Takes a picture of the window.
shoot() {
    import -window "$window" "$out/$1.png"
    echo "Wrote $out/$1.png"
}

for lang in en de; do
    case "$lang" in
    en) locale=en_US.UTF-8 ;;
    de) locale=de_DE.UTF-8 ;;
    esac
    # The plugin takes its language from the locale, see audioref::system_lang.
    LC_ALL="$locale" LANG="$locale" "$program" --backend dummy >"$out/$lang.log" 2>&1 &
    pid=$!
    if ! window=$(timeout 30 xdotool search --sync --name '^AudioRef$' | head -n 1) \
        || [[ -z "$window" ]]; then
        echo "The window didn't open, see $out/$lang.log." >&2
        exit 1
    fi
    sleep 1
    shoot "$lang-empty"

    click "${PATH_FIELD[@]}"
    xdotool type --delay 10 "$database"
    click "${LOAD_BUTTON[@]}"
    sleep 1
    click "${FIRST_HEADPHONES[@]}"
    click "${FIRST_PRESET[@]}"
    sleep 1
    shoot "$lang-correction"

    kill "$pid"
    wait "$pid" 2>/dev/null || true
    pid=
done
