//! The work outside the audio thread: reading OPRA's file and the files of a measurement and a
//! target, designing the corrections, and freeing the ones the audio thread is done with. Without a file, the presets built into AudioRef are
//! used, see `audioref_core::bundled`.
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

use audioref_core::autoeq::{self, Curve};
use audioref_core::bundled;
use audioref_core::cascade::Cascade;
use audioref_core::filter::Filter;
use audioref_core::i18n::Lang;
use audioref_core::opra::{Database, ParseError};

use crate::AudioRefParams;

/// How many corrections fit into each queue: more than the window can ask for between two blocks
/// of audio.
pub const QUEUE: usize = 8;

/// A correction on its way to the audio thread: a filter, or none to turn the correction off.
pub type Correction = Option<Box<Filter>>;

/// What the background does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Task {
    /// Reads and parses OPRA's file at this path, or goes back to the built-in presets.
    Load(Option<PathBuf>),
    /// Reads and parses the file of a measurement or a target.
    LoadCurve(Which, PathBuf),
    /// Designs the saved correction for the current sample rate and sends it to the audio thread.
    Design,
}

/// Where OPRA's file stands.
#[derive(Debug, Clone, Default)]
pub enum Load {
    /// No file: the built-in presets are used.
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

/// One of the two curves a correction is designed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// The measurement of the headphones.
    Measurement,
    /// The target.
    Target,
}

/// Where the file of a curve stands.
#[derive(Debug, Clone, Default)]
pub enum CurveLoad {
    /// No file.
    #[default]
    None,
    /// Read: the curve, with the name of its file.
    Loaded(String, Arc<Curve>),
    /// The file couldn't be read, with the system's reason.
    ReadFailed(String),
    /// The file isn't a curve AudioRef takes.
    Invalid(autoeq::ParseError),
}

/// What the plugin, the background and the window share.
pub struct Shared {
    /// The language of the window, the system's.
    pub lang: Lang,
    /// The sample rate the host last activated the plugin with, as the bits of an `f32`.
    sample_rate: AtomicU32,
    /// The most the limiter lowered the signal since the window last looked, in dB, as the bits
    /// of an `f32`: the audio thread writes it without a lock.
    reduction: AtomicU32,
    /// The highest absolute sample of the output since the window last looked, as the bits of an
    /// `f32`, written by the audio thread without a lock.
    peak: AtomicU32,
    /// OPRA's file.
    pub database: Mutex<Load>,
    /// The presets built into AudioRef, for when no file is loaded.
    pub bundled: Arc<Database>,
    /// The file of the measurement.
    pub measurement: Mutex<CurveLoad>,
    /// The file of the target.
    pub target: Mutex<CurveLoad>,
    /// The end of the queue of new corrections that the background sends into.
    outgoing: Mutex<rtrb::Producer<Correction>>,
    /// The end of the queue of old corrections that the background frees.
    returned: Mutex<rtrb::Consumer<Box<Filter>>>,
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
    pub returns: rtrb::Producer<Box<Filter>>,
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
            reduction: AtomicU32::new(0.0_f32.to_bits()),
            peak: AtomicU32::new(0.0_f32.to_bits()),
            database: Mutex::new(Load::None),
            bundled: Arc::new(bundled::database()),
            measurement: Mutex::new(CurveLoad::None),
            target: Mutex::new(CurveLoad::None),
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

    /// Notes how much the limiter lowered the signal in a block, in dB, keeping the most until
    /// the window takes it. Lock-free, for the audio thread.
    pub fn note_reduction(&self, db: f64) {
        #[expect(clippy::cast_possible_truncation, reason = "a level for the display")]
        let db = db as f32;
        // The bits of floats of 0 or more grow with their values, so the largest bits are the most.
        if db > 0.0 {
            self.reduction.fetch_max(db.to_bits(), Ordering::AcqRel);
        }
    }

    /// The most the limiter lowered the signal since this was last asked, in dB.
    pub fn take_reduction(&self) -> f32 {
        f32::from_bits(self.reduction.swap(0.0_f32.to_bits(), Ordering::AcqRel))
    }

    /// Notes the highest absolute sample of the output in a block, keeping the highest until the
    /// window takes it. Lock-free, for the audio thread.
    pub fn note_peak(&self, peak: f32) {
        // As above: the bits of floats of 0 or more grow with their values.
        if peak > 0.0 {
            self.peak.fetch_max(peak.to_bits(), Ordering::AcqRel);
        }
    }

    /// The highest absolute sample of the output since this was last asked.
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0.0_f32.to_bits(), Ordering::AcqRel))
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
            Task::Load(Some(path)) => self.load(&path),
            Task::Load(None) => *lock(&self.database) = Load::None,
            Task::LoadCurve(which, path) => *lock(self.curve(which)) = load_curve(&path),
            Task::Design => self.send(design(params, self.sample_rate())),
        }
    }

    /// Where the file of a curve stands.
    #[must_use]
    pub fn curve(&self, which: Which) -> &Mutex<CurveLoad> {
        match which {
            Which::Measurement => &self.measurement,
            Which::Target => &self.target,
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

/// Reads and parses the file of a curve, and names it after the file.
fn load_curve(path: &Path) -> CurveLoad {
    match std::fs::read_to_string(path) {
        Err(error) => CurveLoad::ReadFailed(error.to_string()),
        Ok(text) => match Curve::parse(&text) {
            Ok(curve) => {
                let name = path
                    .file_stem()
                    .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
                CurveLoad::Loaded(name, Arc::new(curve))
            }
            Err(error) => CurveLoad::Invalid(error),
        },
    }
}

/// The filter of the saved correction at a sample rate: the FIR of the design if there is one,
/// else the cascade of the preset. None for no correction or one whose values are out of range.
#[must_use]
pub fn design(params: &AudioRefParams, sample_rate: f32) -> Correction {
    let design = params
        .design
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(design) = design {
        let correction = design.correction()?;
        return Filter::fir(&correction, sample_rate).map(Box::new);
    }
    let preset = params
        .correction
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let eq = preset?.eq()?;
    Some(Box::new(Filter::from(Cascade::new(&eq, sample_rate))))
}
