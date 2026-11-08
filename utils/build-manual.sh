#!/usr/bin/env bash
# Renders the user manual, docs/manual/en.md and docs/manual/de.md, as PDF with pandoc and
# LuaLaTeX, in Latin Modern, to target/manual/audioref-manual-en.pdf and -de.pdf. The template and
# the filter for the boxes are in docs/manual. See User manual in docs/DEVELOPMENT.md.
#
# It needs pandoc and TeX Live with LuaLaTeX: utils/install-linux-deps.sh --manual installs them
# on Ubuntu and Debian. pandoc's warnings, e.g. of a missing picture or a character the fonts
# lack, are errors.

set -euo pipefail

cd "$(dirname "$0")/.."

# pandoc reads the texts given below on its command line in the encoding of the locale: UTF-8,
# whatever the system's is.
export LC_ALL=C.UTF-8

for tool in pandoc lualatex; do
    if ! command -v "$tool" > /dev/null; then
        echo "$tool is missing: install it with utils/install-linux-deps.sh --manual." >&2
        exit 1
    fi
done

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
commit=$(git rev-parse --short=7 HEAD 2> /dev/null || echo "${GITHUB_SHA:-unknown}" | cut -c 1-7)
# The date of the commit, for the title page and the PDF's metadata, so the same commit gives the
# same PDF.
SOURCE_DATE_EPOCH=$(git log -1 --format=%ct 2> /dev/null || date +%s)
export SOURCE_DATE_EPOCH FORCE_SOURCE_DATE=1

out=target/manual
mkdir -p "$out"

render() {
    local lang=$1
    shift
    echo "Rendering the manual ($lang) to $out/audioref-manual-$lang.pdf..."
    pandoc "docs/manual/$lang.md" \
        --from markdown-implicit_header_references \
        --to pdf \
        --pdf-engine lualatex \
        --template docs/manual/template.tex \
        --lua-filter docs/manual/manual.lua \
        --resource-path docs/manual \
        --shift-heading-level-by=-1 \
        --top-level-division=chapter \
        --highlight-style tango \
        --fail-if-warnings \
        --metadata version="$version" \
        --variable commit="$commit" \
        --metadata author=Fiona \
        --metadata product=AudioRef \
        --variable title-image="$PWD/docs/screenshots/$lang-correction.png" \
        "$@" \
        --output "$out/audioref-manual-$lang.pdf"
}

render en \
    --metadata lang=en-US \
    --metadata subtitle="User Manual" \
    --metadata tagline="Headphone correction for mixing and mastering, as a VST3 and CLAP plugin" \
    --metadata toc-title="Contents" \
    --metadata version-label=Version \
    --metadata date-label=Date \
    --metadata commit-label=Build \
    --metadata license-note="AudioRef is free software under the GNU General Public License v3.0 only, for private, non-commercial use. Its built-in EQ presets come from OPRA, under CC BY-SA 4.0."

render de \
    --metadata lang=de-DE \
    --metadata subtitle="Benutzerhandbuch" \
    --metadata tagline="Kopfhörerkorrektur zum Mischen und Mastern, als VST3- und CLAP-Plugin" \
    --metadata toc-title="Inhalt" \
    --metadata version-label=Version \
    --metadata date-label=Datum \
    --metadata commit-label=Build \
    --metadata license-note="AudioRef ist freie Software unter der GNU General Public License v3.0 only, für die private, nicht-kommerzielle Nutzung. Seine eingebauten EQ-Presets stammen aus OPRA, unter CC BY-SA 4.0."

ls -l "$out"
