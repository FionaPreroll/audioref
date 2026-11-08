//! What the about window shows besides its texts: the third-party software built into AudioRef,
//! with its licenses, and the time of the build.
//!
//! The plugin's build script runs `cargo metadata`, picks the crates the plugin is built from
//! with [`packages`], reads their license files, which [`is_license_file`] finds, and writes them
//! as Rust with [`code`]; the window shows that list as [`Component`]s.
//!
//! ```
//! use audioref_core::about::{Package, code, utc};
//!
//! let package = Package {
//!     name: "rtrb".to_owned(),
//!     version: "0.3.2".to_owned(),
//!     license: "MIT OR Apache-2.0".to_owned(),
//!     repository: "https://github.com/mgeier/rtrb".to_owned(),
//!     ..Package::default()
//! };
//! let rust = code(&[(package, vec!["MIT License …".to_owned()])]);
//! assert!(rust.contains(r#"name: "rtrb""#));
//! assert_eq!(utc(1_794_156_300), "2026-11-08 16:45 UTC");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use serde::Deserialize;

/// A crate built into AudioRef, as the window shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Component {
    /// Its name, e.g. `egui`.
    pub name: &'static str,
    /// Its version.
    pub version: &'static str,
    /// Its license, as SPDX, e.g. `MIT OR Apache-2.0`.
    pub license: &'static str,
    /// The address of its source; may be empty.
    pub repository: &'static str,
    /// The indices of its license texts in the list of texts that comes with the components.
    pub texts: &'static [usize],
}

/// A crate in `cargo metadata`'s output, ordered by its name and version first.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Package {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// Its license, as SPDX; may be empty.
    pub license: String,
    /// The address of its source; may be empty.
    pub repository: String,
    /// Its authors, for a license without a file of the crate.
    pub authors: Vec<String>,
    /// The folder of its manifest, where its license files are.
    pub dir: String,
}

/// Why `cargo metadata`'s output can't be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataError {
    /// It isn't JSON of the form `cargo metadata --format-version 1` writes; serde's reason.
    NotMetadata(String),
    /// No crate of the workspace has the name asked for.
    NoRoot,
}

/// The most crates read.
pub const MAX_PACKAGES: usize = 10_000;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<RawPackage>,
    resolve: Option<Resolve>,
}

#[derive(Deserialize)]
struct RawPackage {
    id: String,
    name: String,
    version: String,
    license: Option<String>,
    repository: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
    source: Option<String>,
    manifest_path: String,
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    #[serde(default)]
    deps: Vec<Dep>,
}

#[derive(Deserialize)]
struct Dep {
    pkg: String,
    #[serde(default)]
    dep_kinds: Vec<DepKind>,
}

#[derive(Deserialize)]
struct DepKind {
    kind: Option<String>,
}

/// The crates that the workspace's crate `root` is built from, read from the output of `cargo
/// metadata --format-version 1`: those it reaches through normal dependencies, not through
/// those for development or building only, without the crates of the workspace, by name and
/// version.
///
/// # Errors
///
/// [`MetadataError`] if the output can't be read or has no crate `root` of the workspace.
pub fn packages(metadata: &str, root: &str) -> Result<Vec<Package>, MetadataError> {
    let metadata: Metadata = serde_json::from_str(metadata)
        .map_err(|error| MetadataError::NotMetadata(error.to_string()))?;
    let by_id: BTreeMap<&str, &RawPackage> = metadata
        .packages
        .iter()
        .take(MAX_PACKAGES)
        .map(|package| (package.id.as_str(), package))
        .collect();
    let root = by_id
        .values()
        .find(|package| package.name == root && package.source.is_none())
        .ok_or(MetadataError::NoRoot)?;
    let nodes: BTreeMap<&str, &Node> = metadata
        .resolve
        .iter()
        .flat_map(|resolve| &resolve.nodes)
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut reached = BTreeSet::new();
    let mut next = vec![root.id.as_str()];
    while let Some(id) = next.pop() {
        let deps = nodes.get(id).map_or(&[][..], |node| node.deps.as_slice());
        for dep in deps {
            let normal = dep.dep_kinds.iter().any(|kind| kind.kind.is_none());
            if normal && reached.insert(dep.pkg.as_str()) {
                next.push(dep.pkg.as_str());
            }
        }
    }
    let mut found: Vec<Package> = reached
        .into_iter()
        .filter_map(|id| by_id.get(id))
        .filter(|package| package.source.is_some())
        .map(|package| Package {
            name: package.name.clone(),
            version: package.version.clone(),
            license: package.license.clone().unwrap_or_default(),
            repository: package.repository.clone().unwrap_or_default(),
            authors: package.authors.clone(),
            dir: package
                .manifest_path
                .strip_suffix("Cargo.toml")
                .unwrap_or(&package.manifest_path)
                .trim_end_matches(['/', '\\'])
                .to_owned(),
        })
        .collect();
    found.sort();
    found.dedup();
    Ok(found)
}

/// Whether a file of a crate, by its name, holds a license: `LICENSE`, `LICENSE-MIT`,
/// `COPYING`, `NOTICE`, `UNLICENSE`, `COPYRIGHT` and the like, or a text whose name says it is
/// one, e.g. a font's `OFL.txt`.
#[must_use]
pub fn is_license_file(name: &str) -> bool {
    let upper = name.to_uppercase();
    let (stem, extension) = upper.rsplit_once('.').unwrap_or((&upper, ""));
    if !matches!(extension, "" | "TXT" | "MD") || stem.is_empty() {
        return false;
    }
    [
        "LICENSE",
        "LICENCE",
        "COPYING",
        "NOTICE",
        "UNLICENSE",
        "COPYRIGHT",
    ]
    .iter()
    .any(|start| stem.starts_with(start))
        || stem.contains("LICENSE")
        || stem.contains("LICENCE")
        || matches!(stem, "OFL" | "UFL")
}

/// The licenses whose standard text AudioRef brings along, for crates that have no file of
/// their own, by their SPDX identifiers.
pub const STANDARD_LICENSES: [&str; 3] = ["MIT", "Apache-2.0", "ISC"];

/// The licenses of [`STANDARD_LICENSES`] that an SPDX expression names, e.g. `MIT` and
/// `Apache-2.0` for `MIT OR Apache-2.0` or the old `MIT/Apache-2.0`.
#[must_use]
pub fn standard_licenses(expression: &str) -> Vec<&'static str> {
    let named: BTreeSet<&str> = expression
        .split(|c: char| c.is_whitespace() || matches!(c, '/' | '(' | ')'))
        .collect();
    STANDARD_LICENSES
        .into_iter()
        .filter(|license| named.contains(license))
        .collect()
}

/// The text for a crate without license files of its own: whose it is, then a standard text.
#[must_use]
pub fn standard_text(package: &Package, text: &str) -> String {
    let authors = if package.authors.is_empty() {
        format!("the authors of {}", package.name)
    } else {
        package.authors.join(", ")
    };
    format!(
        "{} {}: copyright {authors}.\n\n{text}",
        package.name, package.version
    )
}

/// The crates with their license texts as Rust: the statics `COMPONENTS`, of
/// [`Component`]s, and `TEXTS`, each text once, for the build script to write and the plugin to
/// include.
#[must_use]
pub fn code(packages: &[(Package, Vec<String>)]) -> String {
    let mut texts: Vec<&str> = Vec::new();
    let mut rust = String::from("pub static COMPONENTS: &[audioref_core::about::Component] = &[\n");
    for (package, own) in packages {
        let mut indices = Vec::new();
        for text in own {
            let index = texts
                .iter()
                .position(|known| known == text)
                .unwrap_or_else(|| {
                    texts.push(text);
                    texts.len() - 1
                });
            indices.push(index.to_string());
        }
        // Debug writes a string as a literal of Rust, quoted and escaped.
        let _ = writeln!(
            rust,
            "    audioref_core::about::Component {{ name: {:?}, version: {:?}, license: {:?}, \
             repository: {:?}, texts: &[{}] }},",
            package.name,
            package.version,
            package.license,
            package.repository,
            indices.join(", ")
        );
    }
    rust.push_str("];\n\npub static TEXTS: &[&str] = &[\n");
    for text in texts {
        let _ = writeln!(rust, "    {text:?},");
    }
    rust.push_str("];\n");
    rust
}

/// A time as seconds since 1970 in UTC, e.g. `2026-11-08 16:45 UTC`.
#[must_use]
pub fn utc(seconds: u64) -> String {
    let days = seconds / 86_400;
    let minutes = seconds % 86_400 / 60;
    // Howard Hinnant's days_from_civil, backwards, in eras of 400 years from 0000-03-01.
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        minutes / 60,
        minutes % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small output of `cargo metadata`: the plugin, the core in the workspace, a normal
    /// dependency with its own dependency, one for building only and one for development only.
    const METADATA: &str = r#"{
        "packages": [
            {"id": "audioref 0.1.0 (path+file:///w/crates/audioref)", "name": "audioref", "version": "0.1.0", "license": "GPL-3.0-only", "repository": null, "source": null, "manifest_path": "/w/crates/audioref/Cargo.toml"},
            {"id": "core", "name": "audioref-core", "version": "0.1.0", "license": "GPL-3.0-only", "source": null, "manifest_path": "/w/crates/audioref-core/Cargo.toml"},
            {"id": "b", "name": "b", "version": "1.0.0", "license": "MIT", "repository": "https://b", "authors": ["Bea"], "source": "registry", "manifest_path": "/r/b-1.0.0/Cargo.toml"},
            {"id": "a", "name": "a", "version": "2.0.0", "license": null, "source": "git", "manifest_path": "C:\\r\\a\\Cargo.toml"},
            {"id": "build", "name": "cc", "version": "1.0.0", "license": "MIT", "source": "registry", "manifest_path": "/r/cc/Cargo.toml"},
            {"id": "dev", "name": "approx", "version": "0.5.0", "license": "Apache-2.0", "source": "registry", "manifest_path": "/r/approx/Cargo.toml"},
            {"id": "other", "name": "audioref", "version": "9.0.0", "source": "registry", "manifest_path": "/r/other/Cargo.toml"}
        ],
        "resolve": {"root": null, "nodes": [
            {"id": "audioref 0.1.0 (path+file:///w/crates/audioref)", "deps": [
                {"pkg": "core", "dep_kinds": [{"kind": null}]},
                {"pkg": "b", "dep_kinds": [{"kind": "dev"}, {"kind": null, "target": "cfg(unix)"}]},
                {"pkg": "build", "dep_kinds": [{"kind": "build"}]},
                {"pkg": "dev", "dep_kinds": [{"kind": "dev"}]}
            ]},
            {"id": "core", "deps": [{"pkg": "b", "dep_kinds": [{"kind": null}]}]},
            {"id": "b", "deps": [{"pkg": "a", "dep_kinds": [{"kind": null}]}]},
            {"id": "a"}
        ]}
    }"#;

    #[test]
    fn finds_the_crates_built_into_the_plugin() {
        let found = packages(METADATA, "audioref").unwrap();
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(found[0].dir, "C:\\r\\a");
        assert_eq!(found[0].license, "");
        assert_eq!(found[0].repository, "");
        assert_eq!(
            found[1],
            Package {
                name: "b".to_owned(),
                version: "1.0.0".to_owned(),
                license: "MIT".to_owned(),
                repository: "https://b".to_owned(),
                authors: vec!["Bea".to_owned()],
                dir: "/r/b-1.0.0".to_owned(),
            }
        );
        // The core alone, as a root of its own.
        assert_eq!(packages(METADATA, "audioref-core").unwrap().len(), 2);
    }

    #[test]
    fn reports_what_it_cannot_read() {
        assert_eq!(packages(METADATA, "nothing"), Err(MetadataError::NoRoot));
        assert!(matches!(
            packages("[]", "audioref"),
            Err(MetadataError::NotMetadata(_))
        ));
        // Without the resolved graph, nothing is reached.
        let packages_only = r#"{"packages": [{"id": "r", "name": "r", "version": "1", "source": null, "manifest_path": "Cargo.toml"}]}"#;
        assert_eq!(packages(packages_only, "r"), Ok(vec![]));
    }

    #[test]
    fn knows_license_files_by_their_names() {
        for name in [
            "LICENSE",
            "LICENSE-MIT",
            "license-apache.md",
            "LICENCE.txt",
            "COPYING",
            "NOTICE",
            "UNLICENSE",
            "COPYRIGHT",
            "OFL.txt",
            "UFL.txt",
            "emoji-icon-font-mit-license.txt",
        ] {
            assert!(is_license_file(name), "{name}");
        }
        for name in ["license.rs", "README.md", "Cargo.toml", ".txt", "OFL.ttf"] {
            assert!(!is_license_file(name), "{name}");
        }
    }

    #[test]
    fn brings_standard_texts_for_crates_without_files() {
        assert_eq!(
            standard_licenses("MIT OR Apache-2.0"),
            ["MIT", "Apache-2.0"]
        );
        assert_eq!(standard_licenses("MIT/Apache-2.0"), ["MIT", "Apache-2.0"]);
        assert_eq!(standard_licenses("(ISC)"), ["ISC"]);
        assert_eq!(standard_licenses("Zlib"), Vec::<&str>::new());
        assert_eq!(standard_licenses("MIT-0"), Vec::<&str>::new());
        let mut package = Package {
            name: "egui".to_owned(),
            version: "0.31.1".to_owned(),
            authors: vec!["Emil".to_owned(), "Others".to_owned()],
            ..Package::default()
        };
        assert_eq!(
            standard_text(&package, "MIT"),
            "egui 0.31.1: copyright Emil, Others.\n\nMIT"
        );
        package.authors.clear();
        assert_eq!(
            standard_text(&package, "MIT"),
            "egui 0.31.1: copyright the authors of egui.\n\nMIT"
        );
    }

    #[test]
    fn writes_each_text_once() {
        let package = |name: &str| Package {
            name: name.to_owned(),
            version: "1".to_owned(),
            ..Package::default()
        };
        let rust = code(&[
            (
                package("x"),
                vec!["A \"quoted\"\n".to_owned(), "B".to_owned()],
            ),
            (package("y"), vec!["B".to_owned()]),
            (package("z"), vec![]),
        ]);
        assert_eq!(
            rust,
            "pub static COMPONENTS: &[audioref_core::about::Component] = &[\n    \
             audioref_core::about::Component { name: \"x\", version: \"1\", license: \"\", \
             repository: \"\", texts: &[0, 1] },\n    \
             audioref_core::about::Component { name: \"y\", version: \"1\", license: \"\", \
             repository: \"\", texts: &[1] },\n    \
             audioref_core::about::Component { name: \"z\", version: \"1\", license: \"\", \
             repository: \"\", texts: &[] },\n];\n\n\
             pub static TEXTS: &[&str] = &[\n    \"A \\\"quoted\\\"\\n\",\n    \"B\",\n];\n"
        );
    }

    #[test]
    fn writes_times_in_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00 UTC");
        assert_eq!(utc(1_794_156_300), "2026-11-08 16:45 UTC");
        assert_eq!(utc(1_798_761_599), "2026-12-31 23:59 UTC");
        assert_eq!(utc(4_107_542_400), "2100-03-01 00:00 UTC");
    }
}
