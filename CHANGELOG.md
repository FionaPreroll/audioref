# Changelog

All changes users of AudioRef can notice are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- AudioRef as a VST3 and a CLAP plugin for Windows, macOS (Intel and Apple silicon) and Linux, see [Installing](README.md#installing).
- Headphone correction with OPRA's EQ presets: load OPRA's `database_v1.jsonl`, which you download yourself, search for your headphones and pick a preset, see [Using AudioRef](README.md#using-audioref). The filters follow the presets closely up to the highest frequencies, at every sample rate from 44.1 to 192 kHz, with no latency.
- Automatic headroom: if a preset boosts some frequencies, AudioRef lowers its level by as much, so the correction never makes the signal louder.
- A window, in English and German: the database, the search, the presets of the chosen headphones, the curve of the correction with the preset's author, the parameters, and the notices.
- The project saves the chosen preset, so it opens with the same correction on a computer without OPRA's file. Changing the preset fades over 20 ms.
- An output level from −24 to +6 dB and a bypass. Changes of both fade over 20 ms, so they don't click.
- Parameter names in English and German, in the language of the operating system.
- "Bypass on export", on by default: AudioRef bypasses itself while the host exports offline, so the headphone correction doesn't end up in a mix. Hosts that export in real time don't tell the plugin: bypass AudioRef by hand there.
- The pre-release [`snapshot`](https://github.com/FionaPreroll/audioref/releases/tag/snapshot) with the plugin bundles of the latest commit on `main`.
