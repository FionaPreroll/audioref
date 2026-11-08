#!/usr/bin/env python3
"""Picks the presets AudioRef bundles out of OPRA's database.

Reads OPRA's dist/database_v1.jsonl and writes the lines of the vendors, products and EQ presets
of the headphones in PRODUCTS, unchanged, sorted by kind and ID, to data/opra/presets.jsonl.
See data/opra/README.md.

Usage: utils/bundle-opra-presets.py path/to/database_v1.jsonl
Then set the commit and the date of the database in data/opra/README.md.
"""

import json
import pathlib
import sys

# OPRA's IDs of the headphones AudioRef bundles, with all their modes.
PRODUCTS = {
    "akg::k240_studio",
    "bose::qc25",
    "bose::quietcomfort_25",
    "bose::quietcomfort_25_passive",
    "sony::wh_1000xm4",
    "sony::wh_1000xm4_anc_off",
    "sony::wh_1000xm4_anc_on",
}

KINDS = {"vendor": 0, "product": 1, "eq": 2}


def wanted(entry):
    kind, data = entry.get("type"), entry.get("data", {})
    if kind == "vendor":
        return any(product.split("::")[0] == entry.get("id") for product in PRODUCTS)
    if kind == "product":
        return entry.get("id") in PRODUCTS
    if kind == "eq":
        return data.get("product_id") in PRODUCTS
    return False


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    source = pathlib.Path(sys.argv[1])
    out = pathlib.Path(__file__).resolve().parent.parent / "data" / "opra" / "presets.jsonl"
    picked = []
    for line in source.read_text(encoding="utf-8").splitlines():
        if line.strip():
            entry = json.loads(line)
            if wanted(entry):
                picked.append((KINDS[entry["type"]], entry["id"], line))
    picked.sort()
    found = {entry_id for kind, entry_id, _ in picked if kind == KINDS["product"]}
    if found != PRODUCTS:
        sys.exit(f"Not in the database: {', '.join(sorted(PRODUCTS - found))}")
    with out.open("w", encoding="utf-8", newline="\n") as file:
        for _, _, line in picked:
            file.write(line + "\n")
    presets = sum(1 for kind, _, _ in picked if kind == KINDS["eq"])
    print(f"Wrote {presets} presets for {len(PRODUCTS)} headphones to {out}")


if __name__ == "__main__":
    main()
