# Changelog

All changes users of AudioRef can notice are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.0.0-rc1] - 2026-11-09

The first release candidate of AudioRef 1.0: headphone correction for the monitoring path, as a VST3 and a CLAP plugin for Windows, macOS and Linux, with OPRA's whole database built in, your own measurement and target, linear phase, crossfeed, a limiter, a window and a manual in English and German, and a Windows installer.

### Added

- AudioRef as a VST3 and a CLAP plugin for Windows, macOS (Intel and Apple silicon) and Linux, see [Installing](README.md#installing).
- OPRA's whole database built in, of 8 November 2026: 12,594 EQ presets for 6,230 headphones, by oratory1990 and AutoEQ, used by default, so there is nothing to download or choose. For a newer version, load OPRA's `database_v1.jsonl`, which you download yourself, under **OPRA database**; loading with an empty path goes back to the built-in one.
- Headphone correction with OPRA's EQ presets: search for your headphones and pick a preset, see [Using AudioRef](README.md#using-audioref). The filters follow the presets closely up to the highest frequencies, at every sample rate from 44.1 to 192 kHz, with no latency.
- Automatic headroom: if a preset boosts some frequencies, AudioRef lowers its level by as much, so the correction never makes the signal louder.
- A window, in English and German, in a dark look with a bright purple accent: tabs for OPRA's presets and your own curves, the search, the presets of the chosen headphones, OPRA's database in a section that folds away, the frequency response of the correction EQ with the preset's author and a link to OPRA, the parameters in three groups (output, stereo, protection), and the notices. Rest the pointer on the curve to read the level at that frequency.
- The project saves the chosen preset, so it opens with the same correction on a computer without OPRA's file, and the window shows its headphones and the preset selected. Changing the preset fades over 20 ms.
- An output level from −24 to +6 dB and a bypass. Changes of both fade over 20 ms, so they don't click.
- Parameter names in English and German, in the language of the operating system.
- "Bypass on export", on by default: AudioRef, the VST3 and the CLAP version alike, bypasses itself while the host exports offline, so the headphone correction doesn't end up in a mix. Hosts that export in real time don't tell the plugin: bypass AudioRef by hand there.
- Your own measurement and target: in the tab "Own curves (AutoEQ)", load a measurement of your headphones and a target curve as CSV files, e.g. from AutoEQ's repository, which you download yourself, and use them as the correction, see [Your own measurement and target](README.md#your-own-measurement-and-target). AudioRef smooths it, corrects nothing above 16 kHz, limits boosts and runs it as a minimum-phase filter without latency.
- **Linear phase**, off by default: the correction, a preset or your own curves, runs without shifting the phase, and delays the sound by 43 to 46 ms, which AudioRef reports to the DAW, so it makes up for it in playback. Turned on and off at the top right of the frequency response; the project saves the choice, see [Linear phase](README.md#linear-phase).
- Tilt, bass and the limit for boosts, to adjust a correction from your own curves to your taste. The project saves both curves and the settings, so it opens without the files.
- Mono and swapped channels, under **Channels**: to check a mix in mono, or which side is which.
- Crossfeed in three strengths, off by default: each ear hears some of the other channel's bass, as from speakers, so wide mixes tire less on headphones. What both channels share stays as it is.
- A limiter at the end, on by default: it keeps the output at or below −0.3 dBFS without adding latency, so raising the output level doesn't clip. A bar in the window shows by how much it lowers the signal.
- A display of the output level, left of the limiter: a bar from −60 to 0 dBFS and its peak in dB. It moves whenever sound plays, while the limiter's bar only moves when the limiter has to lower the signal. Click either bar to cross both out and turn them off, e.g. on slow graphics: the window is then only drawn again when something in it changes. The project saves the choice.
- An about window, opened by **About** at the top right or by a click on the name AudioRef: the version, the format the DAW loaded (CLAP or VST3), the git commit and the time of the build, whether GitHub Actions built it, with a link to its run, the contact, the homepage, a link to the releases to download, the licenses of AudioRef and its built-in presets, and every crate AudioRef is built with, with its license text.
- Hints in the window, in English and German: rest the pointer on a control to see what it does. Where the window waits for something, it says what to do next, e.g. to pick headphones, to load both curves, or to lower the output level when the limiter has much to do.
- `cargo xtask fetch-profiles`, for those who build AudioRef from source: downloads OPRA's database, AutoEQ's Harman target and oratory1990's measurements of the headphones you name, the three above by default, into a folder outside the repository, and checks that AudioRef can read them, see [Fetching the files](README.md#fetching-the-files).
- A user manual in English and German, as PDF in every archive and with every release, and as Markdown in [docs/manual](docs/manual/en.md): installing, the window, presets and your own curves, linear phase, the parameters, using AudioRef in your DAW, a reference and troubleshooting.
- DAWs that sort their plugins by category list AudioRef under **Mastering**, the VST3 and the CLAP version alike, as it belongs last on the master track; some also list it under EQ.
- Links in the window's notes: under **OPRA database**, `database_v1.jsonl` downloads OPRA's database in your browser and "OPRA's repository" opens its repository; in the tab "Own curves (AutoEQ)", "AutoEQ's repository" opens AutoEQ's.
- **Browse…** beside the fields of OPRA's database, the measurement and the target: it opens your system's file dialog, in the folder of the file in the field, and AudioRef loads the file you pick at once and remembers its path. Typing or pasting a path still works.
- Each time the window opens, AudioRef reads the files that the project names again: OPRA's database, so the saved preset is selected, and the measurement and the target. If one isn't there, e.g. because it was moved, there is no error: the built-in presets or the saved correction stay, and a note under **OPRA database** says so.
- A Windows installer, `audioref-windows-x86_64-setup.exe`, with every release: its express installation installs the VST3 and the CLAP plugin where DAWs look for them and the user manual in English and German, its custom installation only the ones you choose, and its last page offers to open the manual. It speaks English and German, updates an earlier installation, and removes AudioRef again under **Settings → Apps**, see [Installing](README.md#installing).
- The pre-release [`snapshot`](https://github.com/FionaPreroll/audioref/releases/tag/snapshot) with the plugin bundles of the latest commit on `main`.
