# Architecture

How AudioRef is built inside: its crates, its threads and the path of the signal. Parts marked *planned* come with the next steps of [PLAN.md](PLAN.md#next-steps).

## Crates

| Crate | Holds | Depends on |
|---|---|---|
| `crates/audioref-core` | The logic as plain Rust: the texts (`i18n`), the saved correction (`state`), the signal processing (`engine`, `gain`, the filters in `biquad` and `cascade`; *planned:* the convolution), parametric EQs with the ranges of their values (`eq`), OPRA's database (`opra`) and the presets built in from it (`bundled`), AutoEQ's curves (`autoeq`), the design of a correction from them (`design`) and its minimum-phase filter (`fir`) | No crate with plugin APIs, GUI or I/O; `make lint` checks it |
| `crates/audioref` | The plugin: nih-plug, the parameters, the language of the system, the window (`editor`, with egui), reading OPRA's file and designing the corrections in the background (`background`), the saved state | `audioref-core`, nih-plug, nih_plug_egui, rtrb, sys-locale |
| `xtask` | The bundler, `cargo xtask bundle audioref --release` | nih-plug's xtask |
| `fuzz` | The fuzz targets, a workspace of its own, see [fuzz/README.md](../fuzz/README.md) | `audioref-core`, libfuzzer-sys |
| `screenshots` | The plugin as a standalone program, for screenshots of the window, a workspace of its own, see Screenshots in [DEVELOPMENT.md](DEVELOPMENT.md#screenshots) | `audioref`, nih-plug's standalone host |

The core can be tested without a host, to 100 % of its lines and functions. The plugin crate only translates between nih-plug and the core.

## Threads

```text
 GUI thread (editor)            background                         audio thread
 ───────────────────            ────────────────────               ────────────
 pick headphones, mode,  ──▶    read OPRA's file,          ──▶     correction → crossfeed
 target, controls               design the filters                 → level → limiter
                                (core: opra, biquad,       lock-free (core: engine,
                                 cascade)                  hand-over  cascade)
```

- The audio thread never allocates, locks or reads files. Filters are designed in the background and handed over without locks; the old ones go back to be freed outside the audio thread.
- A new correction cross-fades from the one before for 20 ms, like the level and the bypass (`audioref_core::engine`). The engine keeps the correction before until the next one comes, and hands it back then, so it never frees one itself.
- The window saves the chosen preset and asks the background, a thread of the plugin's own, to design it for the current sample rate (`crates/audioref/src/background.rs`). nih-plug's background thread isn't used: the executor nih-plug hands to the editor holds the plugin's wrapper, so an editor that kept it would keep the plugin alive, and nih-plug's thread would still run when the host unloads the library, which crashes the host on Windows. AudioRef's thread starts with the plugin and is joined when the host frees it. The background sends the cascade through a lock-free queue (rtrb) to the audio thread, which takes it at the start of the next block, and gets the old one back through a second queue to free it. The audio thread takes a correction only while the queue back has room, and leaves out one designed for another sample rate.
- When the host activates the plugin, e.g. with a new sample rate or after loading a project, `initialize` empties the queue and designs the saved correction itself, outside the audio thread, so the first block has it.
- Only the window and the background lock the mutexes of `Shared`: OPRA's database, and the ends of the queues they hold. The audio thread never does.

## The signal

Stereo in, stereo out, with a latency of 0 samples. In this order, the parts not built yet marked *planned*:

1. Automatic headroom: the overall gain of the correction is lowered by its largest boost between 20 Hz and 20 kHz, on AutoEQ's grid, so it boosts nothing (`audioref_core::cascade`). Most of OPRA's presets bring enough of their own; the most any needs is 5.4 dB.
2. The correction: a cascade of biquads for OPRA's presets (`audioref_core::cascade`), computed with 64 bits; *planned:* a minimum-phase FIR for AutoEQ's curves, convolved with zero latency (the first partition in the time domain), growing with the sample rate: 4096 taps at 44.1 and 48 kHz, 8192 at 88.2 and 96 kHz, 16384 at 176.4 and 192 kHz.
   The biquads follow Vicanek's matched design (`audioref_core::biquad`), as the usual formulas bend above about 10 kHz at 44.1 kHz: the poles of the analog filter, and zeros that give the analog magnitude at 0 Hz, at the band's frequency and at half the sample rate. Peaks and shelves up to 10 kHz stay within 0.75 dB of the analog filter up to half the sample rate. Bands at or above half the sample rate are left out.
3. *Planned:* crossfeed, optional, in three strengths.
4. *Planned:* mono and swapped channels.
5. The output level and the bypass, both faded over 20 ms (`audioref_core::engine`), the length adapted to the sample rate.
6. *Planned:* a limiter at −0.3 dBFS, with a display when it acts.

## Headphone data

`audioref_core::opra` reads the text of OPRA's `database_v1.jsonl`, about 12 MB with some 19,500 lines, in about 0.1 s: its vendors, products and EQ presets, each by its ID. OPRA lists the modes of some headphones as products of their own, e.g. "WH-1000XM4 (ANC on)".
A line it doesn't understand, e.g. of a kind a later OPRA adds, a band type it doesn't know or a value outside the ranges of `audioref_core::eq`, is left out and counted, so one bad line doesn't cost the others. The ranges hold for every EQ that reaches the filters: frequencies from 1 Hz to 100 kHz, band gains of ±30 dB, Qs from 0.1 to 100, an overall gain from −60 to +30 dB, and at most 32 bands, the first ones, as OPRA sorts them by priority.
The plugin reads the file in the background, and the window shows each preset's author, as OPRA's license (CC BY-SA 4.0) asks.

Without a file, the window uses the presets built into the plugin (`audioref_core::bundled`): `data/opra/presets.jsonl`, an excerpt of OPRA's database with the Bose QC25, the Sony WH-1000XM4 and the AKG K240 Studio in all their modes, about 20 KB, compiled in with `include_str!` and parsed once when the plugin starts. When a file can't be read, the window says so and keeps the built-in presets; loading with an empty path goes back to them. `data/opra/README.md` names the source, the commit and the authors; `utils/bundle-opra-presets.py` makes the excerpt again from a newer database.

`audioref_core::autoeq` reads measurements and targets in AutoEQ's CSV format, a header `frequency,raw` and one line per frequency, and what other tools write: more columns (it takes `raw`, or else the column after the frequency), no header, `;` with a decimal comma, tabs or spaces, comments starting with `#` or `*`. Like OPRA's database, a line it doesn't understand is left out and counted. A curve has from 2 to 100,000 points with rising frequencies from 1 Hz to 100 kHz and levels of ±200 dB, so sound pressure levels pass too, and must reach from 100 Hz to 10 kHz; between its points the level runs straight on a logarithmic frequency axis, and beyond its ends it stays flat. The fuzz target `autoeq_curve` reads it.

## Designing a correction

`audioref_core::design` computes a correction from a measurement and a target, on AutoEQ's grid of 1.01 steps from 20 Hz to 20 kHz:

1. Correction = target + tilt + bass shelf − measurement. The tilt goes from −1 to +1 dB per octave around 1 kHz; the bass shelf, from −6 to +12 dB, is the Harman target's: half its gain at 105 Hz, Q 0.71.
2. Smoothing: over 1/12 octave up to 2 kHz, wider above, 1/3 octave from 8 kHz, so narrow peaks and dips of a single measurement, which move with the fit, aren't copied.
3. Set to 0 dB on average from 200 Hz to 2 kHz.
4. Faded out from 12 to 16 kHz with a raised cosine, nothing above: up there, measurements depend on the ear and the fit more than on the headphones.
5. Limits: boosts up to +6 dB by default (0 to 12 dB), cuts to −30 dB.
6. *Planned:* a warning when the target doesn't fit the rig of the measurement.

`audioref_core::fir` makes the minimum-phase filter of a correction, by the real cepstrum, with FFTs 8 times as long as the filter (realfft): 4096 taps at 44.1 and 48 kHz, 8192 at 88.2 and 96 kHz, 16384 at 176.4 and 192 kHz, the last eighth faded out. Its overall gain is lowered by the largest boost, so it boosts nothing, like the cascade's. It follows the correction within 0.5 dB from 30 Hz to 20 kHz at every sample rate, and puts 98 % of its energy into the first 2 ms. It takes 2 to 10 ms, so it runs in the background.

## Render guard

With "Bypass on export" on, the default, AudioRef bypasses itself while the host processes offline (`audioref_core::render`). Hosts activate the plugin again whenever that changes, with `ProcessMode::Offline` in nih-plug, so the bypass is in place from the first sample of an export, without a fade. In the VST3 version this needs the fork of nih-plug, which hands `initialize` the mode the host just set, see Dependencies in [PLAN.md](PLAN.md#dependencies). The parameter `bypass_on_export` is saved with the project, but isn't automatable.
Hosts that export in real time don't say so, which is why the window also shows the notice to bypass AudioRef before exporting. Whether Bitwig and FL Studio export offline is checked in them, see Next Steps in [PLAN.md](PLAN.md#next-steps).

## Window

`crates/audioref/src/editor.rs` draws the window with egui, through nih_plug_egui, in the system's language: OPRA's file and whether it loaded, or the built-in presets, the search for headphones, the presets of the chosen ones, the current correction with its author ("EQ by …, from OPRA, CC BY-SA 4.0", and a link to OPRA's repository, as OPRA asks), its headroom and its curve from 20 Hz to 20 kHz, the parameters, and the notices. It only reads and shows; the search, the texts and the curve's values come from the core.
On Linux, the window needs X11 and OpenGL; `utils/install-linux-deps.sh` installs the libraries to build it.

## Saved state

The project saves, besides the parameters:

- `correction`: the chosen preset with its bands, author and headphones (`audioref_core::state::SavedCorrection`), so a project opens with its correction on a computer without OPRA's file. nih-plug stores it as JSON; its values are checked again before they are used, and the fuzz target `saved_correction` reads it.
- `opra-path`: the path of OPRA's file, which the window loads again when it opens; empty for the built-in presets.
- `editor-state`: the size of the window.

Parameter IDs and these keys never change; the names of the parameters are in the system's language.
