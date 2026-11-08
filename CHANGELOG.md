# Changelog

All changes users of AudioRef can notice are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- AudioRef as a VST3 and a CLAP plugin for Windows, macOS (Intel and Apple silicon) and Linux, see [Installing](README.md#installing).
- Built-in EQ presets from OPRA for the Bose QC25, the Sony WH-1000XM4 and the AKG K240 Studio, in all their modes, by oratory1990 and AutoEQ: these headphones work without a download. Loading with an empty path goes back to them.
- Headphone correction with OPRA's EQ presets: for other headphones, load OPRA's `database_v1.jsonl`, which you download yourself, search for your headphones and pick a preset, see [Using AudioRef](README.md#using-audioref). The filters follow the presets closely up to the highest frequencies, at every sample rate from 44.1 to 192 kHz, with no latency.
- Automatic headroom: if a preset boosts some frequencies, AudioRef lowers its level by as much, so the correction never makes the signal louder.
- A window, in English and German: the database, the search, the presets of the chosen headphones, the curve of the correction with the preset's author and a link to OPRA, the parameters, and the notices.
- The project saves the chosen preset, so it opens with the same correction on a computer without OPRA's file. Changing the preset fades over 20 ms.
- An output level from −24 to +6 dB and a bypass. Changes of both fade over 20 ms, so they don't click.
- Parameter names in English and German, in the language of the operating system.
- "Bypass on export", on by default: AudioRef, the VST3 and the CLAP version alike, bypasses itself while the host exports offline, so the headphone correction doesn't end up in a mix. Hosts that export in real time don't tell the plugin: bypass AudioRef by hand there.
- Your own measurement and target: in the tab "Own curves (AutoEQ)", load a measurement of your headphones and a target curve as CSV files, e.g. from AutoEQ's repository, which you download yourself, and use them as the correction, see [Your own measurement and target](README.md#your-own-measurement-and-target). AudioRef smooths it, corrects nothing above 16 kHz, limits boosts and runs it as a minimum-phase filter without latency.
- Tilt, bass and the limit for boosts, to adjust a correction from your own curves to your taste. The project saves both curves and the settings, so it opens without the files.
- The pre-release [`snapshot`](https://github.com/FionaPreroll/audioref/releases/tag/snapshot) with the plugin bundles of the latest commit on `main`.
