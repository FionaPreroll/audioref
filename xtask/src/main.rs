//! `cargo xtask bundle audioref --release` builds the plugin and writes its VST3 and CLAP bundles
//! to `target/bundled`, see Building in `docs/DEVELOPMENT.md`.
//!
//! `cargo xtask fetch-profiles [--to <folder>] [<headphones>…]` downloads OPRA's database,
//! AutoEQ's Harman target and oratory1990's measurements of the headphones into a folder outside
//! the repository, see `profiles`.

mod profiles;

fn main() -> nih_plug_xtask::Result<()> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some(profiles::COMMAND) {
        if let Err(message) = profiles::fetch(args) {
            eprintln!("{message}");
            std::process::exit(1);
        }
        return Ok(());
    }
    nih_plug_xtask::main()
}
