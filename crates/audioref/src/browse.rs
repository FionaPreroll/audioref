//! The system's dialog that picks a file for a field of the window: OPRA's file, a measurement or
//! a target (`audioref_core::browse`).
//!
//! The dialog must not hold up the host. On Windows and Linux it runs on a thread of its own,
//! which hands the file to the window through a mutex the window looks at in each frame; the
//! window's thread goes on drawing, and no message of the host reaches the window while it is in
//! the middle of a frame. The thread is joined when the plugin is freed, so it never outlives the
//! library: if the dialog is still open then, the host waits until it is closed. On macOS the
//! system shows dialogs only from the main thread, which the window runs on: there the dialog is
//! modal, as usual on macOS, and the window waits for it.
//!
//! nih-plug's executor isn't used, for the reason given in [`crate::background`].

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "macos"))]
use std::thread::{self, JoinHandle};

use audioref_core::browse::{ALL_FILES, Pick, folder_of};
use audioref_core::i18n::{Lang, Text};

use crate::background::lock;

/// Where the dialog stands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum Status {
    /// Closed, with nothing for the window.
    #[default]
    Closed,
    /// Open, for a field.
    Open(Pick),
    /// Closed with a file picked for a field, until the window takes it.
    Picked(Pick, PathBuf),
}

/// The dialog of the window, at most one at a time.
#[derive(Default)]
pub struct Dialog {
    /// Where it stands, shared with its thread.
    status: Arc<Mutex<Status>>,
    /// The thread of the last dialog.
    #[cfg(not(target_os = "macos"))]
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Dialog {
    /// Whether a dialog is open: then the window opens no other.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(*lock(&self.status), Status::Open(_))
    }

    /// The file picked for a field, once: the window puts it into the field and loads it.
    #[must_use]
    pub fn take(&self) -> Option<(Pick, PathBuf)> {
        let mut status = lock(&self.status);
        match std::mem::take(&mut *status) {
            Status::Picked(pick, path) => Some((pick, path)),
            other => {
                *status = other;
                None
            }
        }
    }

    /// Opens the system's dialog for a field, in the folder of the file in the field, unless one
    /// is open already.
    pub fn open(&self, pick: Pick, lang: Lang, field: &str) {
        let (name, extensions) = pick.filter(lang);
        let mut dialog = rfd::FileDialog::new()
            .set_title(pick.title().text(lang))
            .add_filter(name, extensions);
        // macOS lists the files of all filters together, so there "all files" would hide none.
        if cfg!(not(target_os = "macos")) {
            dialog = dialog.add_filter(Text::AllFiles.text(lang), ALL_FILES);
        }
        if let Some(folder) = folder_of(field).filter(|folder| folder.is_dir()) {
            dialog = dialog.set_directory(folder);
        }
        self.run(pick, move || dialog.pick_file());
    }

    /// Runs a dialog that picks a file for a field, unless one is open already.
    fn run(&self, pick: Pick, dialog: impl FnOnce() -> Option<PathBuf> + Send + 'static) {
        {
            let mut status = lock(&self.status);
            if matches!(*status, Status::Open(_)) {
                return;
            }
            *status = Status::Open(pick);
        }
        #[cfg(target_os = "macos")]
        finish(&self.status, pick, dialog());
        #[cfg(not(target_os = "macos"))]
        {
            let mut thread = lock(&self.thread);
            // The last dialog was closed, so its thread is done or about to be.
            if let Some(last) = thread.take() {
                drop(last.join());
            }
            let status = self.status.clone();
            let spawned = thread::Builder::new()
                .name(String::from("audioref-dialog"))
                .spawn(move || finish(&status, pick, dialog()));
            match spawned {
                Ok(handle) => *thread = Some(handle),
                // Without a thread, no dialog: the field still takes a path typed in.
                Err(_) => *lock(&self.status) = Status::Closed,
            }
        }
    }
}

/// Notes what a dialog for a field picked, if anything.
fn finish(status: &Mutex<Status>, pick: Pick, path: Option<PathBuf>) {
    *lock(status) = path.map_or(Status::Closed, |path| Status::Picked(pick, path));
}

#[cfg(not(target_os = "macos"))]
impl Drop for Dialog {
    fn drop(&mut self) {
        if let Some(thread) = lock(&self.thread).take() {
            drop(thread.join());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::mpsc;

    use audioref_core::browse::Pick;

    use super::Dialog;

    #[test]
    fn hands_over_the_picked_file_once() {
        let dialog = Dialog::default();
        assert!(!dialog.is_open());
        assert_eq!(dialog.take(), None);
        let (opened, wait) = mpsc::channel::<()>();
        // On macOS the dialog runs in the window's thread, so it must close at once.
        if cfg!(target_os = "macos") {
            let _ = opened.send(());
        }
        dialog.run(Pick::Target, move || {
            // Stays open until the test closes it, like a user who takes their time.
            let _ = wait.recv();
            Some(PathBuf::from("/curves/target.csv"))
        });
        if cfg!(not(target_os = "macos")) {
            assert!(dialog.is_open());
            assert_eq!(dialog.take(), None);
            // A second dialog doesn't open while the first is.
            dialog.run(Pick::Database, || panic!("a second dialog opened"));
        }
        let _ = opened.send(());
        let picked = loop {
            if let Some(picked) = dialog.take() {
                break picked;
            }
            std::thread::yield_now();
        };
        assert_eq!(picked, (Pick::Target, PathBuf::from("/curves/target.csv")));
        assert!(!dialog.is_open());
        assert_eq!(dialog.take(), None);
    }

    #[test]
    fn a_cancelled_dialog_picks_nothing() {
        let dialog = Dialog::default();
        dialog.run(Pick::Measurement, || None);
        while dialog.is_open() {
            std::thread::yield_now();
        }
        assert_eq!(dialog.take(), None);
        // The next one opens.
        dialog.run(Pick::Measurement, || Some(PathBuf::from("m.csv")));
        drop(dialog);
    }
}
