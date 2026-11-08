//! Collects what the about window shows of the build: the git commit, the time, and the crates
//! the plugin is built from with their license texts, see `audioref_core::about`.
//!
//! Nothing here may fail the build: what it can't find, the window shows as unknown.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use audioref_core::about::{
    Package, code, is_license_file, packages, standard_licenses, standard_text,
};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=licenses");
    println!("cargo:rerun-if-changed=../../Cargo.lock");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    let commit = commit(&manifest_dir).unwrap_or_default();
    println!("cargo:rustc-env=AUDIOREF_COMMIT={commit}");

    // A reproducible build sets the time; else it is now.
    let seconds = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|seconds| seconds.trim().parse::<u64>().ok())
        .or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|time| time.as_secs())
        })
        .unwrap_or_default();
    println!("cargo:rustc-env=AUDIOREF_BUILD_TIME={seconds}");

    let components = third_party(&manifest_dir).unwrap_or_default();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default()).join("third_party.rs");
    if let Err(error) = fs::write(&out, code(&components)) {
        println!("cargo:warning=Could not write {}: {error}", out.display());
    }
}

/// The commit of the source, from git, or from GitHub's variable outside a repository.
fn commit(manifest_dir: &Path) -> Option<String> {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(manifest_dir)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        // Builds again when the commit changes.
        let dir = PathBuf::from(dir);
        println!("cargo:rerun-if-changed={}", dir.join("HEAD").display());
        println!(
            "cargo:rerun-if-changed={}",
            dir.join("packed-refs").display()
        );
        if let Some(head) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={}", dir.join(head).display());
        }
    }
    git(&["rev-parse", "HEAD"])
        .or_else(|| env::var("GITHUB_SHA").ok())
        .filter(|commit| !commit.is_empty())
}

/// The crates the plugin is built from, each with its license texts, or `None` if `cargo
/// metadata` fails, e.g. offline without the crates.
fn third_party(manifest_dir: &Path) -> Option<Vec<(Package, Vec<String>)>> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["metadata", "--format-version", "1", "--offline"])
        .current_dir(manifest_dir);
    if let Ok(target) = env::var("TARGET") {
        command.args(["--filter-platform", &target]);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        println!(
            "cargo:warning=cargo metadata failed, the about window lists no crates: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }
    let metadata = String::from_utf8_lossy(&output.stdout);
    let found = match packages(&metadata, env!("CARGO_PKG_NAME")) {
        Ok(found) => found,
        Err(error) => {
            println!("cargo:warning=The output of cargo metadata can't be read: {error:?}");
            return None;
        }
    };
    let standard = manifest_dir.join("licenses");
    Some(
        found
            .into_iter()
            .map(|package| {
                let texts = texts(&package, &standard);
                (package, texts)
            })
            .collect(),
    )
}

/// The license texts of a crate: its license files, in its folder and the folders in it; for a
/// crate from git without any, those at the top of its repository; else the standard texts of
/// its license, with its name and authors.
fn texts(package: &Package, standard: &Path) -> Vec<String> {
    let dir = Path::new(&package.dir);
    let mut texts = license_files(dir, true);
    // The top of cargo's checkout of a repository holds `.cargo-ok`, as does each crate of a
    // registry; up to there, the first license files at the top of a folder.
    let top = dir
        .ancestors()
        .take(6)
        .find(|folder| folder.join(".cargo-ok").exists());
    if let Some(top) = top {
        for folder in dir.ancestors().skip(1) {
            if !texts.is_empty() || !folder.starts_with(top) {
                break;
            }
            texts = license_files(folder, false);
        }
    }
    if texts.is_empty() {
        texts = standard_licenses(&package.license)
            .into_iter()
            .filter_map(|license| fs::read_to_string(standard.join(format!("{license}.txt"))).ok())
            .map(|text| standard_text(package, &text))
            .collect();
    }
    texts
}

/// The texts of the license files in a folder, and if `deep` in the folders in it, by their
/// paths.
fn license_files(dir: &Path, deep: bool) -> Vec<String> {
    let mut paths = Vec::new();
    let entries = fs::read_dir(dir).into_iter().flatten().flatten();
    for entry in entries {
        let path = entry.path();
        if deep && path.is_dir() {
            let inner = fs::read_dir(&path).into_iter().flatten().flatten();
            paths.extend(
                inner
                    .map(|entry| entry.path())
                    .filter(|path| is_license(path)),
            );
        } else if is_license(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .map(|text| text.trim_end().to_owned())
        .filter(|text| !text.is_empty())
        .collect()
}

/// Whether a path is a file that holds a license.
fn is_license(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_license_file)
}
