# Changelog

All changes users of AudioRef can notice are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- AudioRef as a VST3 and a CLAP plugin for Windows, macOS (Intel and Apple silicon) and Linux, see [Installing](README.md#installing). It doesn't correct headphones yet.
- An output level from −24 to +6 dB and a bypass. Changes of both fade over 20 ms, so they don't click.
- Parameter names in English and German, in the language of the operating system.
- "Bypass on export", on by default: AudioRef bypasses itself while the host exports offline, so the headphone correction doesn't end up in a mix. Hosts that export in real time don't tell the plugin: bypass AudioRef by hand there.
- The pre-release [`snapshot`](https://github.com/FionaPreroll/audioref/releases/tag/snapshot) with the plugin bundles of the latest commit on `main`.
