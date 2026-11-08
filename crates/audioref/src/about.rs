//! The about window: the version, the commit and the time of the build, the contact, the
//! homepage, the licenses, and the third-party software with its license texts, which
//! `build.rs` collects.

use audioref_core::about::{Component, utc};
use audioref_core::i18n::{About, Lang};
use nih_plug_egui::egui;

// COMPONENTS and TEXTS, written by build.rs.
include!(concat!(env!("OUT_DIR"), "/third_party.rs"));

/// Whom to write to.
pub const CONTACT_NAME: &str = "Fiona";

/// Where to write to.
pub const CONTACT_ADDRESS: &str = "fipreroll+app@gmail.com";

/// AudioRef's homepage.
pub const HOMEPAGE: &str = "https://fionapreroll.github.io/";

/// The git commit the plugin was built from; empty if unknown.
const COMMIT: &str = env!("AUDIOREF_COMMIT");

/// The time of the build, in seconds since 1970 in UTC; 0 if unknown.
const BUILD_TIME: &str = env!("AUDIOREF_BUILD_TIME");

/// The source's repository, for the link to the commit.
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// Draws the window while `open`; its cross closes it.
pub fn window(ctx: &egui::Context, lang: Lang, open: &mut bool) {
    egui::Window::new(About::Title.text(lang))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_width(600.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| contents(ui, lang));
}

/// What the window shows.
fn contents(ui: &mut egui::Ui, lang: Lang) {
    ui.heading("AudioRef");
    ui.label(About::Description.text(lang));
    ui.add_space(6.0);
    egui::Grid::new("about").num_columns(2).show(ui, |ui| {
        ui.label(About::Version.text(lang));
        ui.label(env!("CARGO_PKG_VERSION"));
        ui.end_row();
        ui.label(About::Commit.text(lang));
        if COMMIT.is_empty() {
            ui.label(About::Unknown.text(lang));
        } else {
            let short = COMMIT.get(..12).unwrap_or(COMMIT);
            ui.hyperlink_to(short, format!("{REPOSITORY}/commit/{COMMIT}"))
                .on_hover_text(COMMIT);
        }
        ui.end_row();
        ui.label(About::Built.text(lang));
        match BUILD_TIME.parse::<u64>() {
            Ok(seconds) if seconds > 0 => ui.label(utc(seconds)),
            _ => ui.label(About::Unknown.text(lang)),
        };
        ui.end_row();
        ui.label(About::Contact.text(lang));
        ui.hyperlink_to(
            format!("{CONTACT_NAME} <{CONTACT_ADDRESS}>"),
            format!("mailto:{CONTACT_ADDRESS}"),
        );
        ui.end_row();
        ui.label(About::Homepage.text(lang));
        ui.hyperlink(HOMEPAGE);
        ui.end_row();
    });
    ui.add_space(6.0);
    ui.label(About::WrittenInRust.text(lang));
    ui.label(About::License.text(lang));
    ui.label(About::Presets.text(lang));
    ui.separator();
    ui.strong(About::ThirdParty.text(lang));
    if COMPONENTS.is_empty() {
        ui.label(About::NoThirdParty.text(lang));
        return;
    }
    ui.label(egui::RichText::new(About::ThirdPartyHint.text(lang)).small());
    egui::ScrollArea::vertical()
        .id_salt("third party")
        .max_height(300.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (index, component) in COMPONENTS.iter().enumerate() {
                third_party(ui, index, component);
            }
        });
}

/// A crate: its name, version and license, and opened its source and license texts.
fn third_party(ui: &mut egui::Ui, index: usize, component: &Component) {
    let title = format!(
        "{} {}   {}",
        component.name, component.version, component.license
    );
    egui::CollapsingHeader::new(title)
        .id_salt(("component", index))
        .show(ui, |ui| {
            if !component.repository.is_empty() {
                ui.hyperlink(component.repository);
            }
            for text in component.texts.iter().filter_map(|&index| TEXTS.get(index)) {
                ui.label(egui::RichText::new(*text).monospace().small());
                ui.add_space(4.0);
            }
        });
}
