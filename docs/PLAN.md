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
- **Not planned for now:** own measurements, room or speaker simulation with BRIRs, correction outside the DAW.

## Headphone data

AudioRef ships no data: users point it to files they downloaded themselves, and `cargo xtask fetch-profiles` will fetch them into a folder outside the repository.

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
| Plugin framework | [nih-plug](https://github.com/robbert-vdh/nih-plug) | commit `de42101` of 2026-09-03 | VST3 and CLAP from the same code, parameters, saved state, background tasks, and a bundler. Not on crates.io, see [Dependencies](#dependencies). |
| GUI | egui through `nih_plug_egui` | egui 0.31 | The quickest to build, good for plotted curves, own knobs can be drawn, actively maintained. Vizia, styled with CSS, is the alternative; nih-plug pins a fork of 2024. The adapter for iced is outdated. |
| FFT | realfft / rustfft | chosen in its step | Minimum-phase FIR by cepstrum, convolution |
| Hand-over to the audio thread | rtrb or triple_buffer | chosen in its step | New filters reach the audio thread without locks |
| Parsing | serde, serde_json; csv | serde 1.0.229, serde_json 1.0.154; csv chosen in its step | OPRA's JSON lines; AutoEQ's CSV files |
| Texts | Typed texts in `audioref-core`: an enum per area, one exhaustive `match` per language | | A missing translation doesn't compile, nothing is parsed at run time, and tests reach every text. |
| System language | sys-locale | 0.3.2 | Pure Rust, reads the locale of Windows, macOS and Linux |

## Dependencies

- **nih-plug from git:** it isn't on crates.io. The workspace pins it to one commit, and `deny.toml` allows exactly the git repositories it pins itself: nih-plug, clap-sys, vst3-sys and reflink (for the bundler). Dependabot doesn't move git pins: check nih-plug for new commits now and then, and bump `rev` in `Cargo.toml` deliberately.
- **GPLv3:** nih-plug is ISC, its VST3 bindings (vst3-sys) are GPLv3, so AudioRef is GPL-3.0-only.
- **atty:** nih-plug's logger, nih_log, depends on atty, which is unmaintained (RUSTSEC-2024-0375). It only asks whether stderr is a terminal; `deny.toml` ignores the advisory until nih-plug drops it.
- **Two bugs of nih-plug that clap-validator 0.4.1 finds**, on every system: loading a random state makes nih-plug reserve as much memory as the first 8 bytes say, which aborts the host (test `state-invalid-random`), and after loading a state nih-plug doesn't ask the host to rescan the parameters' values (`state-reproducibility-basic`, `-binary`, `-buffered`). Hosts only hand back states the plugin saved itself, so the first rarely happens in practice. `release.yml` leaves these four tests out (`CLAP_VALIDATOR_EXCLUDE`), and runs all others; take them back in once nih-plug fixes them.
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

## Next Steps

Each milestone ends with bundles from CI.

1. **Frame (this version, 0.1):** the workspace, a plugin with output level and bypass, CI that builds VST3 and CLAP for Windows, macOS and Linux and checks them with clap-validator and pluginval, the README with the disclaimer.
2. **First correction:** import of OPRA presets, a cascade of biquads, automatic headroom, the render guard (bypass while the host exports offline, checked in Bitwig and FL Studio), and a first window: choosing headphones and mode, the curve, the notice "Private, non-commercial use only".
3. **Own targets:** import of AutoEQ's CSV files and targets, smoothing, a limit for boosts, a minimum-phase FIR with zero-latency convolution, tilt and bass controls.
4. **Comfort:** crossfeed, mono and swapped channels, a limiter with its display, hints in context, `cargo xtask fetch-profiles`.
5. **Later, maybe:** a linear-phase mode that reports its latency, targets that imitate other speakers (translation check), speaker simulation with BRIRs from SOFA files.
