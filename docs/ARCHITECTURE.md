# Architecture

How AudioRef is built inside: its crates, its threads and the path of the signal. Parts marked *planned* come with the next steps of [PLAN.md](PLAN.md#next-steps).

## Crates

| Crate | Holds | Depends on |
|---|---|---|
| `crates/audioref-core` | The logic as plain Rust: the texts (`i18n`), the signal processing (`gain`; *planned:* the filters and the convolution), parametric EQs with the ranges of their values (`eq`), OPRA's database (`opra`); *planned:* AutoEQ's curves and the design of the correction | No crate with plugin APIs, GUI or I/O; `make lint` checks it |
| `crates/audioref` | The plugin: nih-plug, the parameters, the language of the system; *planned:* the GUI, reading files, the saved state, background tasks | `audioref-core`, nih-plug, sys-locale |
| `xtask` | The bundler, `cargo xtask bundle audioref --release` | nih-plug's xtask |
| `fuzz` | The fuzz targets, a workspace of its own, see [fuzz/README.md](../fuzz/README.md) | `audioref-core`, libfuzzer-sys |

The core can be tested without a host, to 100 % of its lines and functions. The plugin crate only translates between nih-plug and the core.

## Threads

```text
 GUI thread (planned)           background (planned)               audio thread
 ───────────────────            ────────────────────               ────────────
 pick headphones, mode,  ──▶    parse the profile,         ──▶     level → correction →
 target, controls               design the filters                 crossfeed → limiter
                                (core: profile, design)    lock-free (core: gain, dsp)
                                                           hand-over
```

- The audio thread never allocates, locks or reads files. Filters are designed in the background and handed over without locks; the old ones go back to be freed outside the audio thread.
- *Planned:* each filter change cross-fades for 20 ms, like the level and the bypass now.

## The signal

Now (0.1): stereo in, stereo out. The output level and the bypass fade over 20 ms (`audioref_core::gain`), the length adapted to the sample rate. Latency: 0 samples.

*Planned,* in this order:

1. Input trim and automatic headroom: the gain is lowered by the largest boost of the correction.
2. The correction: a cascade of biquads for OPRA's presets, or a minimum-phase FIR for AutoEQ's curves, convolved with zero latency (the first partition in the time domain). The FIR grows with the sample rate: 4096 taps at 44.1 and 48 kHz, 8192 at 88.2 and 96 kHz, 16384 at 176.4 and 192 kHz. The biquads follow Vicanek's matched design, as the usual formulas bend above about 10 kHz at 44.1 kHz.
3. Crossfeed, optional, in three strengths.
4. Mono, swapped channels, bypass.
5. A limiter at −0.3 dBFS, with a display when it acts.

## Headphone data

`audioref_core::opra` reads the text of OPRA's `database_v1.jsonl`, about 12 MB with some 19,500 lines, in about 0.1 s: its vendors, products and EQ presets, each by its ID. OPRA lists the modes of some headphones as products of their own, e.g. "WH-1000XM4 (ANC on)".
A line it doesn't understand, e.g. of a kind a later OPRA adds, a band type it doesn't know or a value outside the ranges of `audioref_core::eq`, is left out and counted, so one bad line doesn't cost the others. The ranges hold for every EQ that reaches the filters: frequencies from 1 Hz to 100 kHz, band gains of ±30 dB, Qs from 0.1 to 100, an overall gain from −60 to +30 dB, and at most 32 bands, the first ones, as OPRA sorts them by priority.
*Planned:* the plugin reads the file in the background, and the window shows each preset's author, as OPRA's license (CC BY-SA 4.0) asks.

## Designing a correction (planned)

1. Correction = target + tilt + bass shelf − measurement, on AutoEQ's grid of 1.01 steps.
2. Smoothing: 1/12 octave up to 2 kHz, wider above, 1/3 octave from 8 kHz, so narrow dips of a single measurement aren't copied.
3. Limits: boosts up to +6 dB by default, cuts unlimited, nothing above 16 kHz.
4. A warning when the target doesn't fit the rig of the measurement.

## Render guard (planned)

With "Bypass on export" on, the default, AudioRef bypasses itself while the host processes offline (`ProcessMode::Offline` in nih-plug). Hosts that export in real time don't say so, which is why the window also shows the notice to bypass AudioRef before exporting.

## Saved state (planned)

The chosen profile, with its curve, is saved with the project, so a project opens correctly on a computer without the profile files. Parameter IDs never change; their names are in the system's language.
