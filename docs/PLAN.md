# Plan

What was decided in the planning of AudioRef on 2026-11-07, with the versions of that day, and the next steps. Versions move on: check them again when a step starts.

## Scope

- **Goal:** a VST3 and CLAP plugin that corrects the frequency response of headphones toward a neutral target in the monitoring path of a DAW, so budget and consumer headphones get closer to studio monitoring.
- **Use:** private and non-commercial only, see the disclaimer in the README: oratory1990 prohibits unlicensed commercial use of his measurements.
- **Headphones first:** Bose QC25, Sony WH-1000XM4, AKG K240 Studio. All three are in AutoEQ's oratory1990 measurements and in OPRA.
- **DAWs:** Bitwig and FL Studio. Neither has a monitoring path for effects like Reaper's, so the plugin goes last on the master track.
- **Sample rates:** 44.1 to 192 kHz.
- **ANC modes:** not used so far, but offered where the data has them (ANC on, ANC off, passive).
- **Priority:** usability before refinements of the signal processing. Picking the headphones is enough; everything else has safe defaults.
- **The render guard is essential** (Fiona, 2026-11-08): AudioRef bypasses itself while the host exports offline, so the headphone correction never ends up in a mix, in the VST3 and the CLAP version alike. A host or framework bug that breaks it gets fixed, e.g. in the fork of nih-plug, not worked around by leaving the guard out.
- **Not planned for now:** own measurements, room or speaker simulation with BRIRs, correction outside the DAW.

## Headphone data

AudioRef ships OPRA's whole database, so all of its headphones correct without a download and nothing needs to be chosen (Fiona, 2026-11-09; on 2026-11-08 it was an excerpt for the three headphones above): `data/opra/database_v1.jsonl`, unchanged, under CC BY-SA 4.0, built into the plugin, compressed. A newer version of OPRA's database, or a measurement and a target, users load from files they downloaded themselves, or let `cargo xtask fetch-profiles` fetch them into a folder outside the repository. oratory1990's raw measurements have no license and are not shipped; his EQ presets come through OPRA, where he publishes them himself.

| Source | Format | Holds | Terms |
|---|---|---|---|
| AutoEQ, `measurements/oratory1990/data/over-ear/*.csv` | CSV `frequency,raw`: 695 points from 20 Hz to 19.96 kHz, each 1.01 times the last, 0 dB at 1 kHz | Raw frequency responses, digitized from oratory1990's PDFs | AutoEQ's MIT license covers its code, not the data; oratory1990 prohibits unlicensed commercial use |
| AutoEQ, `targets/*.csv` | The same CSV grid | Target curves, e.g. Harman over-ear 2018 | As above |
| AutoEQ, `measurements/oratory1990/name_index.tsv` | TSV `url, source_name, name, form, rig` | The measurement rig of each model, e.g. GRAS 43AG | As above |
| OPRA, `dist/database_v1.jsonl` | JSON lines of vendors, products and EQs; an EQ is a list of filter bands (`peak_dip`, `low_shelf`, `high_shelf`, …) with frequency, gain and Q, and an overall gain | Ready presets, also per mode (ANC on/off, passive) | CC BY-SA 4.0: name the preset's author, visibly |

The rig matters: the Harman target was defined on a GRAS rig, which oratory1990 uses, so measurement and target fit without a compensation.

## Stack

| Area | Choice | Version on 2026-11-07 | Why |
|---|---|---|---|
| Plugin framework | [nih-plug](https://github.com/robbert-vdh/nih-plug), through AudioRef's [fork](https://github.com/FionaPreroll/nih-plug/tree/audioref) | upstream commit `de42101` of 2026-09-03, plus three fixes | VST3 and CLAP from the same code, parameters, saved state, background tasks, and a bundler. Not on crates.io, see [Dependencies](#dependencies). |
| GUI | egui through `nih_plug_egui` | egui 0.31 | The quickest to build, good for plotted curves, own knobs can be drawn, actively maintained. Vizia, styled with CSS, is the alternative; nih-plug pins a fork of 2024. The adapter for iced is outdated. |
| FFT | realfft (on rustfft) | realfft 3.5.1 | Minimum-phase FIR by cepstrum, convolution. Pure Rust, real-valued FFTs of any length. |
| Hand-over to the audio thread | rtrb or triple_buffer | chosen in its step | New filters reach the audio thread without locks |
| Parsing | serde, serde_json; AutoEQ's CSV by hand | serde 1.0.229, serde_json 1.0.154 | OPRA's JSON lines. AutoEQ's CSV files are two columns of numbers, read with the standard library: the csv crate wouldn't help with the separators and headers of other tools, and would be one more dependency. |
| Texts | Typed texts in `audioref-core`: an enum per area, one exhaustive `match` per language | | A missing translation doesn't compile, nothing is parsed at run time, and tests reach every text. |
| System language | sys-locale | 0.3.2 | Pure Rust, reads the locale of Windows, macOS and Linux |
| File dialog | rfd, without default features, with `xdg-portal` | 0.18.0 | The system's own dialog on Windows, macOS and Linux. On Linux it asks the XDG desktop portal through D-Bus, which it loads at run time, or else runs zenity, so the plugin links no GTK and needs no libraries to build. On macOS, dispatch2 0.3.1 queues it on the main thread after the frame that opens it. |

## Dependencies

- **nih-plug from a fork:** nih-plug isn't on crates.io, and its upstream fixes bugs slowly, so AudioRef uses the fork [FionaPreroll/nih-plug](https://github.com/FionaPreroll/nih-plug), branch `audioref`: upstream's `master` plus the fixes below, each its own commit. The workspace pins the fork to one commit, and `deny.toml` allows exactly the fork and the git repositories it pins itself: clap-sys, vst3-sys, reflink (for the bundler), baseview and egui-baseview. Dependabot doesn't move git pins: check upstream for new commits now and then, merge them into `audioref`, and bump `rev` in `Cargo.toml` deliberately, see [DEVELOPMENT.md](DEVELOPMENT.md#ci). Fixes that upstream takes over drop out of the fork on such a merge.
- **The fork's fixes** (as of 2026-11-08), found in testing and by clap-validator 0.4.1, which now runs all its tests:
  - VST3, the process mode: `setup_processing` handed the plugin the mode of the host's previous call, so in Bitwig 6 the VST3 took playing for an export and the render guard bypassed it, and vice versa. Upstream has no fix for it; the fork's is its own. Upstream's issue #71 (Ableton's limiter on after an export) is probably the same bug.
  - CLAP, loading state: nih-plug reserved as much memory as the first 8 bytes of a state said, so a damaged state aborted the host (`state-invalid-random`). Now it refuses lengths above 256 MiB and reads in chunks. Taken from upstream's open pull request #280 (issue #279).
  - CLAP, after loading state: nih-plug didn't ask the host to rescan the parameters' values, so the host showed the old ones (`state-reproducibility-*`). Also taken from pull request #280; #286 does the same with a weaker length check.
- **GPLv3:** nih-plug is ISC, its VST3 bindings (vst3-sys) are GPLv3, so AudioRef is GPL-3.0-only.
- **atty:** nih-plug's logger, nih_log, depends on atty, which is unmaintained (RUSTSEC-2024-0375). It only asks whether stderr is a terminal; `deny.toml` ignores the advisory until nih-plug drops it.
- **vst3-sys and Rust 1.99:** the macros of vst3-sys end expressions with a semicolon, which Rust 1.99 warns of (`semicolon_in_expressions_from_non_local_macros`) and a later Rust will refuse. `crates/audioref` allows the warning; it needs a fix in vst3-sys before Rust turns it into an error.

## Structure

A Cargo workspace, see [ARCHITECTURE.md](ARCHITECTURE.md):

- `crates/audioref-core`: the logic, parsers, signal processing and texts, without nih-plug, GUI or I/O. `make lint` checks its dependencies.
- `crates/audioref`: the plugin: nih-plug, parameters, saved state, later the GUI, files and background tasks.
- `xtask`: the bundler, `cargo xtask bundle audioref --release`.

## Quality and Security

- 100 % of the lines and functions of `audioref-core` covered by tests; the plugin crate stays thin and is checked by clap-validator (CLAP) and pluginval (VST3, strictness 10) in CI.
- No `unsafe`, no panics outside tests: a panic in a plugin takes the host down.
- Nothing in the audio thread allocates, locks or reads files.
- Every parser of outside input gets a fuzz target.
- Dependencies checked by cargo deny and cargo audit; workflows by zizmor and CodeQL; all actions pinned to commits.

## Tested in DAWs

What Fiona tried by hand, beyond clap-validator and pluginval in CI:

- **Bitwig 6** (2026-11-08): CLAP and VST3; output level and bypass work in both, after the fork's fix of the VST3 process mode. The audio export, offline, came out as expected with both: the render guard bypasses AudioRef.
- **FL Studio on Windows** (2026-11-08): tested, no problems reported.

## Next Steps

Each milestone ends with bundles from CI.

1. **Frame (this version, 0.1):** the workspace, a plugin with output level and bypass, CI that builds VST3 and CLAP for Windows, macOS and Linux and checks them with clap-validator and pluginval, the README with the disclaimer.
2. **First correction:** import of OPRA presets, a cascade of biquads, automatic headroom, the render guard (bypass while the host exports offline, checked in Bitwig and FL Studio), and a first window: choosing headphones and mode, the curve, the notice "Private, non-commercial use only".
3. **Own targets:** import of AutoEQ's CSV files and targets (and asking oratory1990 whether AudioRef may ship his measurements of the three headphones; until he agrees, users import them), smoothing, a limit for boosts, a minimum-phase FIR with zero-latency convolution, tilt and bass controls.
4. **Comfort:** crossfeed, mono and swapped channels, a limiter with its display, hints in context, `cargo xtask fetch-profiles`.
5. **Later, maybe:** a linear-phase mode that reports its latency (done, see Linear phase in [ARCHITECTURE.md](ARCHITECTURE.md#linear-phase)), targets that imitate other speakers (translation check), speaker simulation with BRIRs from SOFA files.
