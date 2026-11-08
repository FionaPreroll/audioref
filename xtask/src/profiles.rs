//! `cargo xtask fetch-profiles`: downloads the files of headphones that users load into
//! AudioRef's window, into a folder outside the repository, as AudioRef ships no measurements:
//!
//! - OPRA's `database_v1.jsonl`, for the tab "OPRA presets";
//! - AutoEQ's Harman over-ear 2018 target, and oratory1990's measurements of the headphones
//!   named, found through AutoEQ's name index, for the tab "Own curves (AutoEQ)".
//!
//! It downloads with curl, which comes with Windows 10 and later, macOS and most Linux systems, so
//! the xtask needs no HTTP library. The addresses and the reading of the index are in
//! `audioref_core::profiles`.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use audioref_core::autoeq::Curve;
use audioref_core::opra::Database;
use audioref_core::profiles::{
    DEFAULT_HEADPHONES, FindError, NAME_INDEX, NameIndex, OPRA_DATABASE_URL, TARGET, url,
};

/// The name of the command.
pub const COMMAND: &str = "fetch-profiles";

/// The folder in the user's home the files go to when none is named.
const DEFAULT_FOLDER: &str = "AudioRef profiles";

/// What the command prints for `--help` and for wrong arguments.
const USAGE: &str = "Usage: cargo xtask fetch-profiles [--to <folder>] [<headphones>...]

Downloads, into a folder outside AudioRef's repository (by default \"AudioRef profiles\" in your
home folder):
  database_v1.jsonl             OPRA's database, for the tab \"OPRA presets\"
  targets/Harman over-ear 2018.csv
  measurements/<headphones>.csv oratory1990's measurements from AutoEQ, for \"Own curves (AutoEQ)\"

<headphones> are names as in AutoEQ, or a part of one, e.g. \"HD 600\" or \"WH-1000XM4\".
Without any: AKG K240 Studio, Bose QuietComfort 25, Sony WH-1000XM4.
oratory1990's measurements are for private, non-commercial use only. Needs curl.";

/// Downloads the files, as the arguments after the command ask.
///
/// # Errors
///
/// A message for the user if the arguments are wrong, the folder lies in the repository, curl
/// fails, a file can't be written, or no measurement fits a name.
pub fn fetch(args: impl Iterator<Item = String>) -> Result<(), String> {
    let Some((folder, mut headphones)) = arguments(args)? else {
        println!("{USAGE}");
        return Ok(());
    };
    if headphones.is_empty() {
        headphones = DEFAULT_HEADPHONES.map(str::to_owned).to_vec();
    }
    let folder = match folder {
        Some(folder) => folder,
        None => home()?.join(DEFAULT_FOLDER),
    };
    let folder =
        std::path::absolute(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    outside_the_repository(&folder)?;

    let measurements = folder.join("measurements");
    let targets = folder.join("targets");
    for dir in [&measurements, &targets] {
        fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    let database = folder.join("database_v1.jsonl");
    download(OPRA_DATABASE_URL, &database)?;
    check(&database, |text| {
        Database::parse(text)
            .map(|database| format!("{} presets", database.presets().len()))
            .map_err(|_| "not OPRA's database".to_owned())
    })?;
    let target_name = Path::new(TARGET)
        .file_name()
        .map_or_else(|| OsString::from("target.csv"), OsString::from);
    let target = targets.join(target_name);
    download(&url(TARGET), &target)?;
    check(&target, read_curve)?;

    let index_file = folder.join("name_index.tsv");
    download(&url(NAME_INDEX), &index_file)?;
    let text = fs::read_to_string(&index_file)
        .map_err(|error| format!("{}: {error}", index_file.display()))?;
    let index = NameIndex::parse(&text);

    let mut missing = Vec::new();
    for name in &headphones {
        match index.find(name) {
            Ok(model) => {
                let file = measurements.join(model.file_name());
                download(&model.url(), &file)?;
                check(&file, read_curve)?;
                if !model.rig.is_empty() {
                    println!("  measured on {}", model.rig);
                }
            }
            Err(FindError::NotFound) => missing.push(format!("No measurement of \"{name}\".")),
            Err(FindError::Ambiguous(names)) => missing.push(format!(
                "\"{name}\" fits several, name one of: {}",
                names.join(", ")
            )),
        }
    }
    println!(
        "\nThe files are in {}. Load them in AudioRef's window. oratory1990's measurements are \
         for private, non-commercial use only; OPRA's presets are under CC BY-SA 4.0.",
        folder.display()
    );
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing.join("\n"))
    }
}

/// What the arguments ask for: the folder, if they name one, and the headphones.
type Request = (Option<PathBuf>, Vec<String>);

/// The folder and the headphones the arguments name, or none for `--help`.
fn arguments(mut args: impl Iterator<Item = String>) -> Result<Option<Request>, String> {
    let mut folder = None;
    let mut headphones = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--to" => {
                let path = args
                    .next()
                    .ok_or_else(|| format!("--to needs a folder.\n\n{USAGE}"))?;
                folder = Some(PathBuf::from(path));
            }
            option if option.starts_with("--") => {
                return Err(format!("Unknown option {option}.\n\n{USAGE}"));
            }
            _ => headphones.push(arg),
        }
    }
    Ok(Some((folder, headphones)))
}

/// The user's home folder.
fn home() -> Result<PathBuf, String> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
        .map(PathBuf::from)
        .ok_or_else(|| "No home folder: name one with --to.".to_owned())
}

/// Refuses a folder in the repository: headphone data is never committed.
fn outside_the_repository(folder: &Path) -> Result<(), String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest.parent() else {
        return Ok(());
    };
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_owned());
    // The nearest part of the folder that exists, with its links resolved.
    let resolved = folder
        .ancestors()
        .find_map(|ancestor| {
            fs::canonicalize(ancestor)
                .ok()
                .map(|real| real.join(folder.strip_prefix(ancestor).unwrap_or(Path::new(""))))
        })
        .unwrap_or_else(|| folder.to_owned());
    if resolved.starts_with(&root) {
        Err(format!(
            "{} is inside AudioRef's repository, which never holds headphone data. Name a folder \
             outside it with --to.",
            folder.display()
        ))
    } else {
        Ok(())
    }
}

/// Reads a downloaded file as AudioRef will, and says what it found, so a file AudioRef can't use
/// shows here already, e.g. after the format changed upstream.
fn check(file: &Path, read: fn(&str) -> Result<String, String>) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?;
    let found = read(&text).map_err(|reason| format!("{}: {reason}", file.display()))?;
    println!("  {found}");
    Ok(())
}

/// What AudioRef reads from the file of a curve.
fn read_curve(text: &str) -> Result<String, String> {
    Curve::parse(text)
        .map(|curve| format!("{} points", curve.points().len()))
        .map_err(|error| format!("not a curve AudioRef takes ({error:?})"))
}

/// Downloads a file with curl, first next to it, so a failed download leaves no half file.
fn download(address: &str, file: &Path) -> Result<(), String> {
    println!("{}", file.display());
    let mut partial = file.as_os_str().to_owned();
    partial.push(".part");
    let partial = PathBuf::from(partial);
    let status = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location"])
        .args(["--retry", "3", "--output"])
        .arg(&partial)
        .arg(address)
        .status()
        .map_err(|error| format!("curl couldn't be started ({error}). Install curl."))?;
    if !status.success() {
        drop(fs::remove_file(&partial));
        return Err(format!("curl couldn't download {address} ({status})."));
    }
    fs::rename(&partial, file).map_err(|error| format!("{}: {error}", file.display()))
}
