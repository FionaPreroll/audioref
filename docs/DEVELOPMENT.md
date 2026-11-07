# Development

How to build, test and release AudioRef, and the conventions of the project. [ARCHITECTURE.md](ARCHITECTURE.md) describes the code, [PLAN.md](PLAN.md) the decisions and next steps.

## Getting Started

Install Rust with [rustup](https://rustup.rs); AudioRef builds with the current stable Rust. The checks need some tools:

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

- Coverage leaves out `crates/audioref` and `xtask` (`COVERAGE_EXCLUDE` in the `Makefile`): a test can't build nih-plug's `Buffer` or `Transport` without `unsafe`, so it can't call `process`, and `xtask` only hands over to nih-plug's bundler. The plugin crate stays a thin translation; clap-validator checks it in CI. Everything that can be tested without a host goes into `audioref-core`.
- The coverage of a generic function is that of its best instantiation, not of all of them together. Functions that the tests need to call with other types, e.g. readers that fail, take trait objects (`&mut dyn Read`) instead.

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
| `codeql.yml` | pull requests, `main`, weekly | CodeQL's analysis of the Rust code and of the workflows |
| `zizmor.yml` | pull requests, `main`, weekly | the workflows, with zizmor: e.g. template injection, permissions, known vulnerable actions |
| `release.yml` | pull requests, `main`, version tags | the tests and the VST3 and CLAP bundles on Windows, macOS (universal, signed ad hoc) and Linux, the CLAP bundles checked with clap-validator; publishes them for `main` as the pre-release `snapshot` and for version tags as releases, see [Changelog and versions](#changelog-and-versions) |

Dependabot proposes updates of the crates and the GitHub Actions once a week, each kind in one pull request, and only for versions at least a week old. It doesn't move nih-plug, which is pinned to a git commit, see Dependencies in [PLAN.md](PLAN.md#dependencies).
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
