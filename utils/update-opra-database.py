#!/usr/bin/env python3
"""Puts a newer version of OPRA's database into AudioRef.

Reads OPRA's dist/database_v1.jsonl, checks that each line is a vendor, a product or an EQ preset
in JSON, and copies the file, unchanged, to data/opra/database_v1.jsonl, which the plugin has
built in. See data/opra/README.md.

Usage: utils/update-opra-database.py path/to/database_v1.jsonl
Then set the commit, the date and the numbers of the database in data/opra/README.md.
"""

import collections
import json
import pathlib
import sys

KINDS = ("vendor", "product", "eq")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    source = pathlib.Path(sys.argv[1])
    out = pathlib.Path(__file__).resolve().parent.parent / "data" / "opra" / "database_v1.jsonl"
    data = source.read_bytes()
    if b"\r" in data:
        sys.exit(f"{source} has Windows line endings: download it again, unchanged.")
    counts = collections.Counter()
    for number, line in enumerate(data.decode("utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            entry = json.loads(line)
        except json.JSONDecodeError as error:
            sys.exit(f"Line {number} isn't JSON: {error}")
        if entry.get("type") not in KINDS or not entry.get("id"):
            sys.exit(f"Line {number} isn't a vendor, a product or an EQ preset")
        counts[entry["type"]] += 1
    out.write_bytes(data)
    print(
        f"Wrote {counts['eq']} presets for {counts['product']} headphones "
        f"of {counts['vendor']} vendors to {out}"
    )


if __name__ == "__main__":
    main()
