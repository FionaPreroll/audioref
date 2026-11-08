## What and why

<!-- What does this change, and why? Link an issue it closes, as "Closes #123". -->

## How to see it

<!-- Where to look: the parameter, the window, the DAW and plugin format. For anything users see,
a screenshot, in English and German: the workflow Screenshots keeps them as an artifact, `make screenshots`
takes them locally. -->

## Tried on

<!-- Where it ran: the DAW and its version (e.g. Bitwig 5.3, FL Studio 2025), VST3 or CLAP, the
system, the sample rate. For crashes or wrong sound, attach the host's log. -->

## Checklist

- [ ] `make check` passes (rustfmt, Clippy, docs, the dependencies of `audioref-core`, the tests with 100 % of lines and functions covered, cargo deny, zizmor), as CI runs it
- [ ] A fixed bug has a test that would catch it again; a new feature has tests that cover all of it
- [ ] A new parser of outside input has a fuzz target
- [ ] Changes users can notice are in `CHANGELOG.md` under `## [Unreleased]`, in the section they belong to (Added, Changed, Fixed or Removed), written for users, in English
- [ ] Every text users see is a typed text in `audioref-core`, in English and German
- [ ] `audioref-core` stays free of nih-plug, GUI and I/O; `crates/audioref` only translates
- [ ] Nothing in the audio thread allocates, locks or reads files
- [ ] Headphone data only in `data/opra/`, from OPRA, with its authors named; the plugin IDs and parameter IDs are unchanged
- [ ] No `unsafe`; a new dependency is needed, maintained and passes `cargo deny`
- [ ] The README and `docs/` still say how things work
