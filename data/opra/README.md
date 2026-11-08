# OPRA's presets in AudioRef

`presets.jsonl` holds the EQ presets that AudioRef has built in, so it corrects these headphones without any download:

| Headphones | Modes in OPRA |
|---|---|
| AKG K240 Studio | |
| Bose QC25, QuietComfort 25 | QuietComfort 25 (passive) |
| Sony WH-1000XM4 | WH-1000XM4 (ANC on), WH-1000XM4 (ANC Off) |

## Source and license

The file is an excerpt of `dist/database_v1.jsonl` of [OPRA](https://github.com/opra-project/OPRA), the open, community-maintained directory of headphone EQs, at commit [`0b88ecd`](https://github.com/opra-project/OPRA/blob/0b88ecd4e2bef7cf69fd5d50f1d06fb586c10865/dist/database_v1.jsonl) of 2026-11-08: the lines of the vendors, products and EQ presets of the headphones above, unchanged, sorted by kind and ID. Nothing else was changed.

OPRA's data, and so this excerpt, is licensed under the [Creative Commons Attribution-ShareAlike 4.0 International License](https://creativecommons.org/licenses/by-sa/4.0/) (CC BY-SA 4.0). It is not covered by AudioRef's GPLv3: whoever passes it on, changed or not, does so under CC BY-SA 4.0, naming the authors.

The presets are by:

- **oratory1990**, from his own measurements: AKG K240 Studio, Bose QC25 (also with ANC off and passive), Sony WH-1000XM4 (also for its graphic EQ). His PDFs are linked in each preset.
- **AutoEQ** (Jaakko Pasanen), computed from measurements by oratory1990, crinacle, Innerfidelity, Rtings and HypetheSonics, as each preset names.

AudioRef's window names the author of the chosen preset, then OPRA with a link to its repository and the license, as OPRA asks.

## Updating

Download the current `database_v1.jsonl` from OPRA's repository, then run:

```sh
utils/bundle-opra-presets.py path/to/database_v1.jsonl
```

and set the commit and the date above. The script lists the headphones it picks; add others there.
