# Development

How to build, test and release AudioRef, and the conventions of the project. [ARCHITECTURE.md](ARCHITECTURE.md) describes the code, [PLAN.md](PLAN.md) the decisions and next steps.

## Getting Started

Install Rust with [rustup](https://rustup.rs); AudioRef builds with the current stable Rust. On Linux, the window needs the libraries of X11, XCB and OpenGL: `utils/install-linux-deps.sh` installs them on Ubuntu and Debian, as CI does. The checks need some tools:

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov cargo-deny
pipx install zizmor   # optional: make check uses it if it is installed
```

The `Makefile` holds the commands that CI runs, e.g. `make test`, `make lint` and `make bundle`, with a list of all its targets at the top.
Before pushing, run everything a pull request must pass:

```sh
make check
```

It checks the formatting (`cargo fmt`), runs Clippy and builds the docs with all warnings as errors, checks that `audioref-core` depends on no crate with plugin APIs, GUI or I/O, runs the tests with the coverage check, checks the dependencies, and audits the workflows if zizmor is installed.

## Building

```sh
make bundle
```

builds the plugin in release mode and writes `AudioRef.vst3` and `AudioRef.clap` to `target/bundled`, through nih-plug's bundler (`cargo xtask bundle audioref --release`). On macOS, `cargo xtask bundle-universal audioref --release` writes one bundle for Intel and Apple silicon, as CI does; it needs `rustup target add x86_64-apple-darwin aarch64-apple-darwin`. Copy the bundles where your DAW looks for plugins, see Installing in the README.

## Tests and Coverage

`make coverage` runs the tests with [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) and fails if they cover less than 100 % of the lines or of the functions (`COVERAGE_MIN` in the `Makefile`).
It prints a summary per file, writes an HTML report with the covered lines to `target/llvm-cov/html` and an LCOV report to `target/llvm-cov/lcov.info`.

- Coverage leaves out `crates/audioref` and `xtask` (`COVERAGE_EXCLUDE` in the `Makefile`): a test can't build nih-plug's `Buffer` or `Transport` without `unsafe`, so it can't call `process`, and `xtask` only hands over to nih-plug's bundler. The plugin crate stays a thin translation; clap-validator and pluginval check it in CI. Everything that can be tested without a host goes into `audioref-core`.
- The coverage of a generic function is that of its best instantiation, not of all of them together. Functions that the tests need to call with other types, e.g. readers that fail, take trait objects (`&mut dyn Read`) instead.

## Fuzzing

The fuzz targets in `fuzz/` feed random input to the parsers of what AudioRef reads from outside: OPRA's database, and the correction a project saves.
They need a nightly toolchain and [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz); `make fuzz` runs each of them for a minute, see [fuzz/README.md](../fuzz/README.md).
Every new parser of input from outside gets a target, with a smoke test in the crate's `src/fuzz.rs` that `cargo test` runs.

## Screenshots

`make screenshots` takes pictures of the window without a DAW and without a graphics card, on Linux: `en-built-in.png` (the built-in presets), `en-correction.png` (one of them chosen), `en-file.png` (a preset of a loaded file) and the same in German, in `target/screenshots`. Install what it needs once with `utils/install-linux-deps.sh --screenshots`. It works in CI and in the cloud sandbox of Claude Code alike.

- `screenshots/` holds AudioRef as a standalone program, with nih-plug's host for standalone plugins (`nih_export_standalone`), in a workspace and with a `Cargo.lock` of its own, like `fuzz/`, so the plugin's dependencies stay free of its audio backends. It runs with `--backend dummy`, without sound.
- `utils/screenshots.sh` starts it on Xvfb, a virtual X server, where Mesa's llvmpipe draws the OpenGL. xdotool clicks the first built-in headphones and their first preset, then types the path of `screenshots/demo_database.jsonl`, a small synthetic database in OPRA's format, clicks Load, the first headphones and their first preset again, and ImageMagick's `import` takes the pictures. The language follows the locale it sets.
- nih-plug's window asks for a framebuffer with sRGB, which Mesa's software rendering doesn't offer under Xvfb, so the window wouldn't open ("Could not fetch framebuffer config"). The script copies nih-plug, at the commit `Cargo.toml` pins, to `screenshots/target/nih-plug`, asks for a framebuffer without sRGB there, and `screenshots/Cargo.toml` patches nih-plug with the copy. egui turns the sRGB conversion off anyway, so the window looks the same as in a DAW. The plugin itself is built with nih-plug unchanged.
- The clicks go to fixed points of the window (at the top of the script). When the layout in `crates/audioref/src/editor.rs` changes, check that the pictures still show a chosen preset, and move the points.
- When the plugin's dependencies change, Cargo updates `screenshots/Cargo.lock` to match: commit that change too.

The workflow `screenshots.yml` takes them for every pull request and keeps them as the artifact `screenshots` of its run, for the pull request's "How to see it".

## Dependency Checks

[cargo-deny](https://github.com/EmbarkStudios/cargo-deny) checks the dependencies for known vulnerabilities, unmaintained crates, licenses that don't go with the GPL, and sources other than crates.io and the git repositories that nih-plug pins, as configured in `deny.toml`:

```sh
make deny
```

## CI

| Workflow | Runs on | Checks |
|---|---|---|
| `lint.yml` | pull requests, `main` | rustfmt, Clippy, rustdoc and the dependencies of the core, with a pinned Rust version, so new lints don't break unrelated changes |
| `rust-build.yml` | pull requests, `main` | the debug build |
| `rust-test.yml` | pull requests, `main` | the tests |
| `coverage.yml` | pull requests, `main` | 100 % of lines and functions of the core; the summary is on the run's page, the reports are an artifact and go to Codecov |
| `deny.yml`, `audit.yml` | pull requests, `main`, weekly | the dependencies, with cargo-deny and cargo-audit |
| `fuzz.yml` | pull requests (30 s per target), weekly (15 min), by hand | the fuzz targets |
| `screenshots.yml` | pull requests, `main`, by hand | that the window opens, without a graphics card; the screenshots in English and German are an artifact, see [Screenshots](#screenshots) |
| `codeql.yml` | pull requests, `main`, weekly | CodeQL's analysis of the Rust code and of the workflows |
| `zizmor.yml` | pull requests, `main`, weekly | the workflows, with zizmor: e.g. template injection, permissions, known vulnerable actions |
| `release.yml` | pull requests, `main`, version tags | the tests and the VST3 and CLAP bundles on Windows, macOS (universal, signed ad hoc) and Linux, the CLAP bundles checked with clap-validator, the VST3 bundles with pluginval at strictness 10; publishes them for `main` as the pre-release `snapshot` and for version tags as releases, see [Changelog and versions](#changelog-and-versions) |

clap-validator and pluginval are pinned in `release.yml` (pluginval's archives by their SHA-256), as Dependabot doesn't update them. Four tests of clap-validator are left out because of two bugs of nih-plug, see Dependencies in [PLAN.md](PLAN.md#dependencies).

Dependabot proposes updates of the crates, those of the fuzz targets and the GitHub Actions once a week, each kind in one pull request, and only for versions at least a week old. It doesn't move nih-plug, which is pinned to a git commit, see Dependencies in [PLAN.md](PLAN.md#dependencies).
All GitHub Actions are pinned to the SHA of their commit, and no workflow gets more permissions than it needs.

Settings of the repository that the workflows rely on, made once by an admin:

- **Rulesets** (Settings → Rules → Rulesets): a ruleset for `main` that requires a pull request and the checks above.
- **Codecov:** the [Codecov app](https://github.com/apps/codecov) installed for the repository, and the repository activated on codecov.io; until then, only the upload fails, not the check.
- **Release immutability** (Settings → General → Releases) must stay off: the pre-release `snapshot` takes the files of each new commit on `main`.

## Branches and pull requests

- Every feature or fix gets its own branch off `origin/main`, named `feature/<name_in_snake_case>` or `fix/<name_in_snake_case>`, for example `feature/opra_import` or `fix/bypass_click`, and one pull request against `main`. One feature or fix a branch keeps a pull request easy to review and to take back.
- Nobody pushes to `main`: changes reach it by pull request only.
- A pull request starts from `.github/pull_request_template.md`: what changes and why, how to see it, where it was tried, and a checklist of the project's rules.
- CI runs on every pull request (see above); it must pass before the pull request is merged.

## Changelog and versions

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); changes users can notice go under `## [Unreleased]`, in English. Changes only developers notice, e.g. of tests, tooling or CI, need no entry.
A version is made only when asked, as described below. Versions follow [Semantic Versioning](https://semver.org/).

To release a version, e.g. 0.1.0:

1. On a branch `feature/release_0_1_0`, run `make version VERSION=0.1.0`. It sets the version in `Cargo.toml` and `Cargo.lock`, and turns `## [Unreleased]` into `## [0.1.0] - <today>` with a new, empty `## [Unreleased]` above.
2. Describe the version in a sentence at the start of its section, and merge the pull request.
3. Tag the merge commit on `main` and push the tag: `git tag v0.1.0 && git push origin v0.1.0`.
4. `release.yml` checks that the tag matches the version and is on `main`, builds the bundles, and publishes the release with the version's section of the changelog as notes. A version with a suffix, e.g. `1.0.0-rc.1`, becomes a pre-release.
