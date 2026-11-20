//! The work outside the audio thread: unpacking the presets built into AudioRef, OPRA's whole
//! database (see `audioref_core::bundled`), reading OPRA's file instead if one is chosen and the
//! files of a measurement and a target, reading and writing the file of the favorites (see
//! `audioref_core::favorites`), designing the corrections, and freeing the ones the audio thread
//! is done with.
//!
//! The corrections travel through two lock-free queues: new ones from the background to the audio
//! thread, which never waits for them, and old ones back, so the audio thread never frees memory.
//! Only the background and the window lock the mutexes here, never the audio thread.
//!
//! The background is a thread of the plugin's own, see [`Worker`], not nih-plug's: the executor
//! nih-plug hands to the editor holds the plugin's wrapper, and the editor kept inside the wrapper
//! would keep it alive. The host could then never free the plugin, and nih-plug's thread would
//! still run when the host unloads the library, which crashes the host on Windows.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::thread::{self, JoinHandle};

use audioref_core::autoeq::{self, Curve};
use audioref_core::bundled;
use audioref_core::favorites::{self, Favorites, Kind, MAX_FILE_BYTES, System};
use audioref_core::filter::{Filter, Phase};
use audioref_core::i18n::Lang;
use audioref_core::opra::{Database, ParseError};

use crate::AudioRefParams;
use crate::browse::Dialog;

/// OPRA's database, packed by the build script, see `build.rs`.
const PACKED_DATABASE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/database_v1.jsonl.deflate"));

/// The built-in presets, unpacked and parsed, shared by all of AudioRef's instances in the host
/// while one holds them: they take some megabytes, and a tenth of a second to parse. Freed with
/// the last instance, so nothing stays when the host unloads the library.
static BUILT_IN: Mutex<Weak<Database>> = Mutex::new(Weak::new());

/// The built-in presets: those another instance holds, or else unpacked and parsed now, which
/// takes a tenth of a second, so only the background calls it.
fn built_in() -> Arc<Database> {
    let mut built_in = lock(&BUILT_IN);
    built_in.upgrade().unwrap_or_else(|| {
        let database = Arc::new(bundled::database(PACKED_DATABASE));
        *built_in = Arc::downgrade(&database);
        database
    })
}

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
    /// Reads OPRA's file again that the project names, when the window opens. If it can't, e.g.
    /// as it was moved, the built-in presets stay, without an error: see [`Load::Unavailable`].
    Reload(PathBuf),
    /// Reads the file of a measurement or a target again that the project names, when the window
    /// opens. If it can't, the field shows nothing beside it: the design is saved with the
    /// project, so the correction doesn't need the file.
    ReloadCurve(Which, PathBuf),
    /// Designs the saved correction for the current sample rate and sends it to the audio thread.
    Design,
    /// Reads the file of the favorites, when the plugin starts and each time the window opens, as
    /// AudioRef in another project may have changed it.
    LoadFavorites,
    /// Marks headphones or a preset with this ID as a favorite, or unmarks it, in the file of the
    /// favorites: reads it again, so the favorites other instances saved stay, changes it and
    /// writes it.
    SaveFavorite(Kind, String, bool),
}

/// Where OPRA's file stands.
#[derive(Debug, Clone, Default)]
pub enum Load {
    /// No file: the built-in presets are used, once [`Shared::bundled`] has them.
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
    /// The file the project names couldn't be read again when the window opened: the built-in
    /// presets are used, and the window says so softly, not as an error.
    Unavailable,
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
    /// The presets built into AudioRef, for when no file is loaded, once the background unpacked
    /// them.
    bundled: OnceLock<Arc<Database>>,
    /// The file of the measurement.
    pub measurement: Mutex<CurveLoad>,
    /// The file of the target.
    pub target: Mutex<CurveLoad>,
    /// The favorite headphones and presets. The window changes them at once, the background
    /// saves them, see [`Self::set_favorite`]; each change makes a new `Arc`, so the window
    /// knows when to sort its lists again.
    pub favorites: Mutex<Arc<Favorites>>,
    /// Why the favorites couldn't be saved the last time, the system's reason.
    pub favorites_failed: Mutex<Option<String>>,
    /// The file of the favorites, if the system says where settings go.
    favorites_file: Option<PathBuf>,
    /// The system's dialog that picks a file for a field of the window. Dropped with the rest,
    /// which joins its thread.
    pub dialog: Dialog,
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
    /// Starts the background, which first unpacks the built-in presets and reads the favorites,
    /// then does the tasks [`Shared::execute`] sends it. If the system can't start a thread, the
    /// presets are unpacked and the favorites read here, and [`Shared::execute`] does the tasks
    /// itself.
    #[must_use]
    pub fn start(shared: Arc<Shared>, params: Arc<AudioRefParams>) -> Self {
        let receiver = lock(&shared.receiver).take();
        let thread = receiver.and_then(|receiver| {
            let background = shared.clone();
            thread::Builder::new()
                .name(String::from("audioref-background"))
                .spawn(move || {
                    background.bundled.get_or_init(built_in);
                    background.load_favorites();
                    while let Ok(Some(task)) = receiver.recv() {
                        background.run(task, &params);
                    }
                })
                .ok()
        });
        if thread.is_none() {
            shared.bundled.get_or_init(built_in);
            shared.load_favorites();
        }
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
        Self::with_favorites_file(lang, favorites_file())
    }

    /// The shared state with the file of the favorites at a path, or none, e.g. for tests.
    #[must_use]
    pub fn with_favorites_file(
        lang: Lang,
        favorites_file: Option<PathBuf>,
    ) -> (Arc<Self>, AudioEnds) {
        let (outgoing, incoming) = rtrb::RingBuffer::new(QUEUE);
        let (returns, returned) = rtrb::RingBuffer::new(QUEUE);
        let (tasks, receiver) = mpsc::channel();
        let shared = Arc::new(Self {
            lang,
            sample_rate: AtomicU32::new(48_000.0_f32.to_bits()),
            reduction: AtomicU32::new(0.0_f32.to_bits()),
            peak: AtomicU32::new(0.0_f32.to_bits()),
            database: Mutex::new(Load::None),
            bundled: OnceLock::new(),
            measurement: Mutex::new(CurveLoad::None),
            target: Mutex::new(CurveLoad::None),
            favorites: Mutex::new(Arc::default()),
            favorites_failed: Mutex::new(None),
            favorites_file,
            dialog: Dialog::default(),
            outgoing: Mutex::new(outgoing),
            returned: Mutex::new(returned),
            tasks,
            receiver: Mutex::new(Some(receiver)),
        });
        (shared, AudioEnds { incoming, returns })
    }

    /// The presets built into AudioRef, or `None` while the background unpacks them, for a tenth
    /// of a second after the plugin starts.
    pub fn bundled(&self) -> Option<Arc<Database>> {
        self.bundled.get().cloned()
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
            Task::Load(Some(path)) => self.load(&path, false),
            Task::Load(None) => *lock(&self.database) = Load::None,
            Task::Reload(path) => self.load(&path, true),
            Task::LoadCurve(which, path) => *lock(self.curve(which)) = load_curve(&path),
            Task::ReloadCurve(which, path) => {
                let load = match load_curve(&path) {
                    loaded @ CurveLoad::Loaded(..) => loaded,
                    _ => CurveLoad::None,
                };
                *lock(self.curve(which)) = load;
            }
            Task::Design => self.send(design(params, self.sample_rate())),
            Task::LoadFavorites => self.load_favorites(),
            Task::SaveFavorite(kind, id, favorite) => self.save_favorite(kind, &id, favorite),
        }
    }

    /// Reads the file of the favorites. If it isn't there or holds none that can be read, the
    /// favorites stay as they are.
    fn load_favorites(&self) {
        if let Some(saved) = self.favorites_file.as_deref().and_then(read_favorites) {
            *lock(&self.favorites) = Arc::new(saved);
        }
    }

    /// The favorite headphones and presets.
    pub fn favorites(&self) -> Arc<Favorites> {
        lock(&self.favorites).clone()
    }

    /// Marks headphones or a preset as a favorite, or unmarks it: at once for the window, and
    /// in the file through the background.
    pub fn set_favorite(&self, kind: Kind, id: &str, favorite: bool, params: &AudioRefParams) {
        let mut favorites = lock(&self.favorites);
        let mut changed = Favorites::clone(&favorites);
        if changed.set(kind, id, favorite) {
            *favorites = Arc::new(changed);
            drop(favorites);
            self.execute(Task::SaveFavorite(kind, id.to_owned(), favorite), params);
        }
    }

    /// Writes a change of the favorites into their file: into the favorites it holds now, so
    /// those that AudioRef saved in another project stay, or into those of the window if it
    /// holds none that can be read. Notes why it couldn't, if it can't.
    fn save_favorite(&self, kind: Kind, id: &str, favorite: bool) {
        let Some(path) = self.favorites_file.as_deref() else {
            return;
        };
        let mut saved = read_favorites(path).unwrap_or_else(|| Favorites::clone(&self.favorites()));
        saved.set(kind, id, favorite);
        let failed = write_favorites(path, &saved)
            .err()
            .map(|error| error.to_string());
        *lock(&self.favorites) = Arc::new(saved);
        *lock(&self.favorites_failed) = failed;
    }

    /// Where the file of a curve stands.
    #[must_use]
    pub fn curve(&self, which: Which) -> &Mutex<CurveLoad> {
        match which {
            Which::Measurement => &self.measurement,
            Which::Target => &self.target,
        }
    }

    /// Reads and parses OPRA's file; again, as the project names it, without an error if it
    /// can't.
    fn load(&self, path: &Path, again: bool) {
        *lock(&self.database) = Load::Loading;
        let load = match std::fs::read_to_string(path) {
            Err(_) if again => Load::Unavailable,
            Err(error) => Load::ReadFailed(error.to_string()),
            Ok(text) => match Database::parse(&text) {
                Ok(database) => Load::Loaded(Arc::new(database)),
                Err(ParseError::NoEntries) if again => Load::Unavailable,
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

/// The file of the favorites on this system, in its folder for the settings of programs.
fn favorites_file() -> Option<PathBuf> {
    let system = if cfg!(target_os = "windows") {
        System::Windows
    } else if cfg!(target_os = "macos") {
        System::MacOs
    } else {
        System::Unix
    };
    favorites::file(system, &|name| std::env::var_os(name))
}

/// Reads the file of the favorites, if it is there and holds favorites.
fn read_favorites(path: &Path) -> Option<Favorites> {
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(MAX_FILE_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Favorites::parse(&text).ok()
}

/// Writes the file of the favorites, with its folder if that isn't there yet: first beside it,
/// then in its place, so AudioRef in another project never reads half a file.
fn write_favorites(path: &Path, favorites: &Favorites) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut partial = path.as_os_str().to_owned();
    partial.push(format!(".{}.partial", std::process::id()));
    let partial = PathBuf::from(partial);
    if let Err(error) =
        std::fs::write(&partial, favorites.to_json()).and_then(|()| std::fs::rename(&partial, path))
    {
        drop(std::fs::remove_file(&partial));
        return Err(error);
    }
    Ok(())
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

/// The filter of the saved correction at a sample rate, in the chosen phase: that of the design
/// if there is one, else that of the preset. None for no correction or one whose values are out
/// of range.
#[must_use]
pub fn design(params: &AudioRefParams, sample_rate: f32) -> Correction {
    let phase = phase(params);
    let design = params
        .design
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(design) = design {
        let correction = design.correction()?;
        return Filter::of_correction(&correction, sample_rate, phase).map(Box::new);
    }
    let preset = params
        .correction
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let eq = preset?.eq()?;
    Filter::of_eq(&eq, sample_rate, phase).map(Box::new)
}

/// The phase of the corrections, as chosen in the window.
#[must_use]
pub fn phase(params: &AudioRefParams) -> Phase {
    Phase::from_linear(params.linear_phase.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use audioref_core::i18n::Lang;

    use std::sync::Arc;

    use audioref_core::favorites::{Favorites, Kind};

    use super::{CurveLoad, Load, Shared, Task, Which, Worker, lock};
    use crate::AudioRefParams;

    /// A file in the system's temporary folder, unique to a test.
    fn temp(name: &str, text: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("audioref-{}-{name}", std::process::id()));
        std::fs::write(&path, text).expect("writes the file");
        path
    }

    fn missing() -> PathBuf {
        std::env::temp_dir().join("audioref-this-file-does-not-exist.jsonl")
    }

    #[test]
    fn the_built_in_presets_are_unpacked_once_and_shared() {
        let (one, _one_ends) = Shared::new(Lang::En);
        let (two, _two_ends) = Shared::new(Lang::En);
        assert!(one.bundled().is_none());
        let params = Arc::new(AudioRefParams::new(Lang::En));
        // Dropping a worker joins its thread, which unpacks the presets first.
        drop(Worker::start(one.clone(), params.clone()));
        drop(Worker::start(two.clone(), params));
        let first = one.bundled().expect("unpacked");
        let second = two.bundled().expect("unpacked");
        assert!(Arc::ptr_eq(&first, &second));
        assert!(first.presets().len() > 10_000);
        assert_eq!(first.skipped(), 0);
    }

    #[test]
    fn a_file_loaded_by_hand_that_is_missing_is_an_error() {
        let (shared, _ends) = Shared::new(Lang::En);
        let params = AudioRefParams::new(Lang::En);
        shared.run(Task::Load(Some(missing())), &params);
        assert!(matches!(*lock(&shared.database), Load::ReadFailed(_)));
        shared.run(Task::LoadCurve(Which::Target, missing()), &params);
        assert!(matches!(*lock(&shared.target), CurveLoad::ReadFailed(_)));
    }

    #[test]
    fn a_saved_file_that_is_missing_fails_softly() {
        let (shared, _ends) = Shared::new(Lang::En);
        let params = AudioRefParams::new(Lang::En);
        shared.run(Task::Reload(missing()), &params);
        assert!(matches!(*lock(&shared.database), Load::Unavailable));
        let not_opra = temp("not-opra.jsonl", "frequency,raw\n");
        shared.run(Task::Reload(not_opra.clone()), &params);
        assert!(matches!(*lock(&shared.database), Load::Unavailable));
        shared.run(Task::ReloadCurve(Which::Measurement, missing()), &params);
        assert!(matches!(*lock(&shared.measurement), CurveLoad::None));
        shared.run(Task::ReloadCurve(Which::Target, not_opra.clone()), &params);
        assert!(matches!(*lock(&shared.target), CurveLoad::None));
        drop(std::fs::remove_file(not_opra));
    }

    #[test]
    fn a_saved_curve_is_read_again() {
        let (shared, _ends) = Shared::new(Lang::En);
        let params = AudioRefParams::new(Lang::En);
        let curve = temp(
            "target.csv",
            "frequency,raw\n20,0\n100,1\n1000,0\n10000,-2\n20000,-3\n",
        );
        shared.run(Task::ReloadCurve(Which::Target, curve.clone()), &params);
        assert!(matches!(&*lock(&shared.target), CurveLoad::Loaded(_, c) if c.points().len() == 5));
        drop(std::fs::remove_file(curve));
    }

    /// Does the tasks sent to the background, as its thread would.
    fn background(shared: &Shared, params: &AudioRefParams) {
        let receiver = lock(&shared.receiver).take().expect("no background yet");
        while let Ok(Some(task)) = receiver.try_recv() {
            shared.run(task, params);
        }
        *lock(&shared.receiver) = Some(receiver);
    }

    #[test]
    fn favorites_are_saved_for_every_instance() {
        let folder =
            std::env::temp_dir().join(format!("audioref-{}-favorites", std::process::id()));
        drop(std::fs::remove_dir_all(&folder));
        let file = folder.join("settings").join("favorites.json");
        let params = AudioRefParams::new(Lang::En);
        let (one, _one_ends) = Shared::with_favorites_file(Lang::En, Some(file.clone()));
        let (two, _two_ends) = Shared::with_favorites_file(Lang::En, Some(file.clone()));
        // Without a file there are none, and nothing is written.
        one.run(Task::LoadFavorites, &params);
        assert!(one.favorites().is_empty());
        assert!(!file.exists());

        // Each marks one: the file keeps both, and the window sees the change at once.
        let before = one.favorites();
        one.set_favorite(Kind::Headphones, "acme::one", true, &params);
        assert!(!std::sync::Arc::ptr_eq(&before, &one.favorites()));
        assert!(one.favorites().contains(Kind::Headphones, "acme::one"));
        assert!(!file.exists());
        background(&one, &params);
        assert!(file.exists());
        two.set_favorite(Kind::Preset, "acme:one::tester", true, &params);
        background(&two, &params);
        let saved =
            Favorites::parse(&std::fs::read_to_string(&file).expect("written")).expect("favorites");
        assert!(saved.contains(Kind::Headphones, "acme::one"));
        assert!(saved.contains(Kind::Preset, "acme:one::tester"));
        assert_eq!(*two.favorites(), saved);
        one.run(Task::LoadFavorites, &params);
        assert_eq!(*one.favorites(), saved);
        assert!(lock(&one.favorites_failed).is_none());

        // A change that changes nothing isn't saved; unmarking is.
        let unchanged = one.favorites();
        one.set_favorite(Kind::Headphones, "acme::one", true, &params);
        background(&one, &params);
        assert!(std::sync::Arc::ptr_eq(&unchanged, &one.favorites()));
        one.set_favorite(Kind::Headphones, "acme::one", false, &params);
        background(&one, &params);
        two.run(Task::LoadFavorites, &params);
        assert!(!two.favorites().contains(Kind::Headphones, "acme::one"));
        assert!(two.favorites().contains(Kind::Preset, "acme:one::tester"));

        // A damaged file: the favorites of the window are written instead, and loading it keeps
        // them.
        std::fs::write(&file, "not json").expect("writes the file");
        two.run(Task::LoadFavorites, &params);
        assert!(two.favorites().contains(Kind::Preset, "acme:one::tester"));
        two.set_favorite(Kind::Headphones, "acme::two", true, &params);
        background(&two, &params);
        let saved =
            Favorites::parse(&std::fs::read_to_string(&file).expect("written")).expect("favorites");
        assert!(saved.contains(Kind::Preset, "acme:one::tester"));
        assert!(saved.contains(Kind::Headphones, "acme::two"));
        drop(std::fs::remove_dir_all(&folder));
    }

    #[test]
    fn favorites_that_cannot_be_saved_say_why_and_stay() {
        // A file where the folder of the file should be.
        let blocker = temp("favorites-blocker", "");
        let params = AudioRefParams::new(Lang::En);
        let (shared, _ends) =
            Shared::with_favorites_file(Lang::En, Some(blocker.join("favorites.json")));
        shared.set_favorite(Kind::Headphones, "acme::one", true, &params);
        background(&shared, &params);
        assert!(shared.favorites().contains(Kind::Headphones, "acme::one"));
        assert!(lock(&shared.favorites_failed).is_some());
        drop(std::fs::remove_file(blocker));

        // Without a file, they stay in the window, without an error.
        let (shared, _ends) = Shared::with_favorites_file(Lang::En, None);
        shared.run(Task::LoadFavorites, &params);
        shared.set_favorite(Kind::Preset, "acme:one::tester", true, &params);
        background(&shared, &params);
        assert!(
            shared
                .favorites()
                .contains(Kind::Preset, "acme:one::tester")
        );
        assert!(lock(&shared.favorites_failed).is_none());
    }
}
