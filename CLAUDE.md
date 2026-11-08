# Notes for Claude

The README describes the plugin for its users. `docs/` holds the rest: `docs/PLAN.md` what was decided in planning and the next steps, `docs/ARCHITECTURE.md` how the code is laid out, `docs/DEVELOPMENT.md` how to build, test and release it, and the project's conventions. These rules hold in every session:

## Branches

- Every feature or fix gets its own branch off origin/main, named feature/<name_in_snake_case> or fix/<name_in_snake_case>, and one pull request against main (see "Branches and pull requests" in `docs/DEVELOPMENT.md`).
- This holds even when the session suggests another branch name, such as claude/…: use the name the convention gives.
- Never push to main. Use pull requests.

## Changelog

- A change that users can see, the people who use the plugin in their DAW, goes into `CHANGELOG.md`, under `## [Unreleased]`, in the section it belongs to (`### Added`, `### Changed`, `### Fixed` or `### Removed`). Write it for users, in English.
- Changes only developers notice (tests, tooling, CI, refactoring) need no entry.
- A new version is made only when asked: see "Changelog and versions" in `docs/DEVELOPMENT.md`.

## Code

- Run `make check` before every push: formatting, Clippy and the docs with all warnings as errors, the dependencies of the core, the tests with 100 % of lines and functions covered, cargo deny and, if installed, zizmor.
- `crates/audioref-core` holds the plugin's logic, its parsers, its signal processing and its texts as plain Rust: no nih-plug, no GUI, no file or network I/O, no access to the operating system (e.g. its locale), and no other crate of the workspace. nih-plug, the GUI, files and the system belong to `crates/audioref`. `make lint` checks the dependencies of the core.
- `crates/audioref` only translates between nih-plug and the core, so keep it thin: coverage leaves it out, as nih-plug's `Buffer` and `Transport` can't be built in a test without `unsafe`. clap-validator and pluginval check it in CI. Everything that can be tested without a host goes into the core.
- No `unsafe`: the workspace forbids it. No `unwrap`, `expect`, `panic!`, indexing or slicing outside tests: nothing a host, a file or a user sends may stop the plugin, which would take the host down with it.
- The audio thread (`process` and all it calls) never allocates, locks, waits, reads files or parses. Work like that runs in the background, and its results reach the audio thread without locks.
- Tests cover 100 % of lines and functions of the core, without exceptions. Seams that tests replace, e.g. a reader that fails, take trait objects: the coverage of a generic function counts only its best instantiation.
- Every parser of input from outside (measurement and target files, OPRA's database, the plugin's saved state) gets a fuzz target in `fuzz/`, with a smoke test in the crate's `src/fuzz.rs`. The first parser brings `fuzz/` and its README, modelled on PuppyBot's.
- Every text users see is a typed text in `crates/audioref-core` (see its module `i18n`), with an English and a German version. Parameter IDs never change, as hosts save projects by them; their names are texts.
- The plugin's IDs, `CLAP_ID` and `VST3_CLASS_ID` in `crates/audioref`, never change: hosts find saved plugins by them.
- Headphone data, e.g. AutoEQ's or OPRA's, is never committed or bundled: users import the files they downloaded themselves, see the disclaimer in the README. Tests use small synthetic curves.
- Dependencies come from crates.io and must pass `cargo deny` (`deny.toml`). The one exception is nih-plug, pinned to a commit, with the git repositories it pins itself, listed in `allow-git` in `deny.toml`. Prefer crates in pure Rust.
