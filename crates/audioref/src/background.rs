//! The work outside the audio thread: reading OPRA's file, designing the corrections, and freeing
//! the ones the audio thread is done with.
//!
//! The corrections travel through two lock-free queues: new ones from the background to the audio
//! thread, which never waits for them, and old ones back, so the audio thread never frees memory.
//! Only the background and the window lock the mutexes here, never the audio thread.
//!
//! The background is a thread of the plugin's own, see [`Worker`], not nih-plug's: the executor
//! nih-plug hands to the editor holds the plugin's wrapper, and the editor kept inside the wrapper
//! would keep it alive. The host could then never free the plugin, and nih-plug's thread would
//! still run when the host unloads the library, which crashes the host on Windows.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use audioref_core::cascade::Cascade;
use audioref_core::i18n::Lang;
use audioref_core::opra::{Database, ParseError};
use audioref_core::state::SavedCorrection;

use crate::AudioRefParams;

/// How many corrections fit into each queue: more than the window can ask for between two blocks
/// of audio.
pub const QUEUE: usize = 8;

/// A correction on its way to the audio thread: a cascade, or none to turn the correction off.
pub type Correction = Option<Box<Cascade>>;

/// What the background does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Task {
    /// Reads and parses OPRA's file at this path.
    Load(PathBuf),
    /// Designs the saved correction for the current sample rate and sends it to the audio thread.
    Design,
}

/// Where OPRA's file stands.
#[derive(Debug, Clone, Default)]
pub enum Load {
    /// Not loaded yet.
    #[default]
    None,
    /// Being read.
    Loading,
    /// Read: the database.
    Loaded(Arc<Database>),
    /// The file couldn't be read, with the system's reason.
    ReadFailed(String),
    /// The file isn't OPRA's database.
    NotOpra,
}

/// What the plugin, the background and the window share.
pub struct Shared {
    /// The language of the window, the system's.
    pub lang: Lang,
    /// The sample rate the host last activated the plugin with, as the bits of an `f32`.
    sample_rate: AtomicU32,
    /// OPRA's file.
    pub database: Mutex<Load>,
    /// The end of the queue of new corrections that the background sends into.
    outgoing: Mutex<rtrb::Producer<Correction>>,
    /// The end of the queue of old corrections that the background frees.
    returned: Mutex<rtrb::Consumer<Box<Cascade>>>,
    /// The tasks for the background, `None` to stop it.
    tasks: Sender<Option<Task>>,
    /// The other end of `tasks`, until the background takes it.
    receiver: Mutex<Option<Receiver<Option<Task>>>>,
}

/// The background thread: started with the plugin, stopped and joined when the plugin is dropped,
/// so it never outlives the library.
pub struct Worker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Starts the background, which does the tasks [`Shared::execute`] sends it. If the system
    /// can't start a thread, [`Shared::execute`] does them itself.
    #[must_use]
    pub fn start(shared: Arc<Shared>, params: Arc<AudioRefParams>) -> Self {
        let receiver = lock(&shared.receiver).take();
        let thread = receiver.and_then(|receiver| {
            let background = shared.clone();
            thread::Builder::new()
                .name(String::from("audioref-background"))
                .spawn(move || {
                    while let Ok(Some(task)) = receiver.recv() {
                        background.run(task, &params);
                    }
                })
                .ok()
        });
        Self { shared, thread }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            // The background holds `shared`, and with it the sender, so it is told to stop.
            drop(self.shared.tasks.send(None));
            drop(thread.join());
        }
    }
}

/// The ends of the queues that the audio thread holds.
pub struct AudioEnds {
    /// New corrections, from the background.
    pub incoming: rtrb::Consumer<Correction>,
    /// Old corrections, back to the background.
    pub returns: rtrb::Producer<Box<Cascade>>,
}

/// Locks a mutex, also after a thread panicked while holding it: what it guards stays usable.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    /// The shared state, and the ends of the queues for the audio thread.
    #[must_use]
    pub fn new(lang: Lang) -> (Arc<Self>, AudioEnds) {
        let (outgoing, incoming) = rtrb::RingBuffer::new(QUEUE);
        let (returns, returned) = rtrb::RingBuffer::new(QUEUE);
        let (tasks, receiver) = mpsc::channel();
        let shared = Arc::new(Self {
            lang,
            sample_rate: AtomicU32::new(48_000.0_f32.to_bits()),
            database: Mutex::new(Load::None),
            outgoing: Mutex::new(outgoing),
            returned: Mutex::new(returned),
            tasks,
            receiver: Mutex::new(Some(receiver)),
        });
        (shared, AudioEnds { incoming, returns })
    }

    /// The sample rate the host last activated the plugin with, in Hz.
    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::Acquire))
    }

    /// Notes the sample rate the host activates the plugin with.
    pub fn set_sample_rate(&self, sample_rate: f32) {
        self.sample_rate
            .store(sample_rate.to_bits(), Ordering::Release);
    }

    /// Has the background do a task. Without a background, as when the system couldn't start
    /// it, the task is done here.
    pub fn execute(&self, task: Task, params: &AudioRefParams) {
        if let Err(mpsc::SendError(Some(task))) = self.tasks.send(Some(task)) {
            self.run(task, params);
        }
    }

    /// Does a task, on the background thread.
    pub fn run(&self, task: Task, params: &AudioRefParams) {
        match task {
            Task::Load(path) => self.load(&path),
            Task::Design => {
                let saved = params
                    .correction
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                self.send(design(saved.as_ref(), self.sample_rate()));
            }
        }
    }

    /// Reads and parses OPRA's file.
    fn load(&self, path: &Path) {
        *lock(&self.database) = Load::Loading;
        let load = match std::fs::read_to_string(path) {
            Err(error) => Load::ReadFailed(error.to_string()),
            Ok(text) => match Database::parse(&text) {
                Ok(database) => Load::Loaded(Arc::new(database)),
                Err(ParseError::NoEntries) => Load::NotOpra,
            },
        };
        *lock(&self.database) = load;
    }

    /// Frees the corrections the audio thread sent back, and sends it a new one. If the queue is
    /// full, as when the host doesn't process, the correction is dropped: the host designs it
    /// again when it activates the plugin.
    fn send(&self, correction: Correction) {
        let mut returned = lock(&self.returned);
        while returned.pop().is_ok() {}
        drop(returned);
        // A full queue hands the correction back, which is dropped here, outside the audio thread.
        drop(lock(&self.outgoing).push(correction));
    }
}

/// The cascade of a saved correction at a sample rate, none for no correction or one whose values
/// are out of range.
#[must_use]
pub fn design(saved: Option<&SavedCorrection>, sample_rate: f32) -> Correction {
    let eq = saved?.eq()?;
    Some(Box::new(Cascade::new(&eq, sample_rate)))
}
