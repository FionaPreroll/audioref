# Fuzzing

The fuzz targets feed random input to the code that reads what AudioRef gets from outside: OPRA's database of headphone presets, and the correction that a project saves.
Every parser of input from outside, e.g. measurement and target files or the plugin's saved state, gets a target of its own.
They run with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and libFuzzer.

| Target | Input | Checks beyond not crashing |
|---|---|---|
| `opra_database` | The text of OPRA's `database_v1.jsonl` | Every preset that is accepted has at most 32 bands, and its gains, frequencies and Qs in their ranges; no ID is empty; every line that isn't empty is either an entry or counted as skipped. |
| `saved_correction` | The JSON of the correction in the plugin's saved state | A correction that is used has its values in their ranges, and its filters at 44.1 kHz a finite response that boosts nothing on the grid where the headroom is measured. |

## Running

The fuzz targets need a nightly toolchain and cargo-fuzz:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
```

Run each target for a minute, or for longer with `FUZZ_SECONDS`:

```sh
make fuzz
make fuzz FUZZ_SECONDS=600
```

Or run a single target until you stop it:

```sh
cd fuzz
cargo +nightly fuzz run opra_database -- -dict=audioref.dict
```

[`audioref.dict`](audioref.dict) holds tokens of the input, e.g. the fields of OPRA's database and the kinds of its bands, which help libFuzzer find valid input.
The inputs it found go to `corpus/<target>`, and are used again in the next run.
A real `database_v1.jsonl` in `corpus/opra_database` gives libFuzzer a good start; it stays out of the repository, which holds only the excerpt in `data/opra/` (copy `data/opra/presets.jsonl` there for a small start).

## When a Target Fails

libFuzzer saves the input that made the target fail in `artifacts/<target>/`, and prints the command that runs the target with it again:

```sh
cargo +nightly fuzz run opra_database artifacts/opra_database/crash-<hash>
```

`cargo +nightly fuzz tmin opra_database artifacts/opra_database/crash-<hash>` shortens the input. Once the bug is fixed, add the input to the smoke test of the target in the crate's `src/fuzz.rs`, so it stays fixed.

## How It Works

- The functions the targets call are in the `src/fuzz.rs` of the crate whose parser they feed, so far [`crates/audioref-core/src/fuzz.rs`](../crates/audioref-core/src/fuzz.rs). It is compiled with `--cfg fuzzing`, which cargo fuzz sets, and for the tests.
- `cargo test` runs each of these functions on a few inputs, as a smoke test that needs neither nightly nor cargo-fuzz.
- The fuzz crate has a workspace and a `Cargo.lock` of its own, so the `Cargo.lock` of AudioRef stays free of fuzzing dependencies.
- CI ([`.github/workflows/fuzz.yml`](../.github/workflows/fuzz.yml)) runs each target for 30 seconds on pull requests and for 15 minutes once a week. It can also be started by hand, with the time per target. If a target fails, the input is uploaded as the artifact `fuzz-artifacts`.

## Adding a Target

1. Add a function to the crate's `src/fuzz.rs` that feeds its input to the parser and checks what it can, with a smoke test. A crate without one yet gets the module like `crates/audioref-core`: `#[cfg(any(test, fuzzing))] pub mod fuzz;`, and becomes a dependency in `Cargo.toml`.
2. Add `fuzz_targets/<target>.rs`, which calls it, like the others.
3. Add a `[[bin]]` section for it to `Cargo.toml`.
