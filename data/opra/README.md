# OPRA's database in AudioRef

`database_v1.jsonl` is OPRA's whole database, which AudioRef has built in, so it corrects all of its headphones without any download: 12,594 EQ presets for 6,230 headphones of 714 vendors, over-ear, on-ear, in-ear and earbuds. OPRA lists the modes of some headphones as products of their own, e.g. "WH-1000XM4 (ANC on)" and "WH-1000XM4 (ANC Off)".

## Source and license

The file is `dist/database_v1.jsonl` of [OPRA](https://github.com/opra-project/OPRA), the open, community-maintained directory of headphone EQs, at commit [`0b88ecd`](https://github.com/opra-project/OPRA/blob/0b88ecd4e2bef7cf69fd5d50f1d06fb586c10865/dist/database_v1.jsonl) of 2026-11-08, unchanged. The plugin holds it compressed with deflate, and unpacks it when it starts.

OPRA's data, and so this file, is licensed under the [Creative Commons Attribution-ShareAlike 4.0 International License](https://creativecommons.org/licenses/by-sa/4.0/) (CC BY-SA 4.0). It is not covered by AudioRef's GPLv3: whoever passes it on, changed or not, does so under CC BY-SA 4.0, naming the authors.

The presets are by:

- **oratory1990**, from his own measurements, with his PDFs linked in each preset: 1,342 presets.
- **AutoEQ** (Jaakko Pasanen), computed from measurements by crinacle, Innerfidelity, Rtings, Super Review, oratory1990, ToneDeafMonk, Jaytiss, Headphone.com, Hi End Portable, HypetheSonics and others, as each preset names: 11,252 presets, 468 of them named "Rtings/AutoEQ".

AudioRef's window names the author of the chosen preset and what it is based on, then OPRA with a link to its repository and the license, as OPRA asks.

## Updating

Download the current `database_v1.jsonl` from OPRA's repository, then run:

```sh
utils/update-opra-database.py path/to/database_v1.jsonl
```

and set the commit, the date and the numbers above. The script checks that each line is a vendor, a product or an EQ preset and copies the file unchanged; the tests then check that AudioRef understands every line.
