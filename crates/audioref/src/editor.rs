//! The window, drawn with egui, in two tabs: OPRA's file or the built-in presets, the search for
//! headphones and their presets; or the files of a measurement and a target. Below them the
//! correction with its curve and, for a measurement and a target, its settings; the parameters
//! and the notices.
//!
//! Only reading and showing: the files are read and the correction designed in the background,
//! see [`crate::background`].

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use audioref_core::autoeq::ParseError;
use audioref_core::cascade::{Cascade, grid};
use audioref_core::design::{BASS_DB, MAX_BOOST_DB, Settings, TILT_DB_PER_OCTAVE};
use audioref_core::gain::gain_to_db;
use audioref_core::i18n::{About, CurveMessage, Hint, LEVEL_FLOOR_DB, Lang, Message, Notice, Text};
use audioref_core::opra::Database;
use audioref_core::state::{SavedCorrection, SavedDesign};
use audioref_core::stereo::{Channels, Crossfeed};
use nih_plug::prelude::{BoolParam, Editor, IntParam, Param, ParamSetter};
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use nih_plug_egui::{EguiState, create_egui_editor, widgets};

use crate::background::{CurveLoad, Load, Shared, Task, Which, lock};
use crate::{AudioRefParams, about, theme};

/// The size of the window, in logical pixels.
pub const SIZE: (u32, u32) = (760, 720);

/// The height of the lists of headphones and presets, in logical pixels: the built-in
/// headphones fit.
const LIST_HEIGHT: f32 = 140.0;

/// The most headphones the search lists.
const RESULTS: usize = 200;

/// OPRA's repository, linked under the preset's author, as OPRA asks.
const OPRA_URL: &str = "https://github.com/opra-project/OPRA";

/// The height of the cards of the controls, so they end together.
const CONTROLS_HEIGHT: f32 = 92.0;

/// Room beside a slider of a design for its value, e.g. "0.00 dB/oct".
const SLIDER_VALUE_WIDTH: f32 = 110.0;
/// The shortest slider of a design.
const SLIDER_MIN_WIDTH: f32 = 40.0;

/// The least and most height of the curve, in logical pixels: it takes the room left.
const CURVE_MIN_HEIGHT: f32 = 100.0;
const CURVE_MAX_HEIGHT: f32 = 320.0;

/// The range of the curve, in dB: a correction boosts nothing, see the headroom.
const CURVE_DB: (f32, f32) = (6.0, -24.0);

/// The most reduction of the limiter the display shows, in dB.
const METER_DB: f32 = 12.0;

/// How fast the display falls back after the limiter acted, in dB per second.
const METER_FALL_DB_PER_SECOND: f32 = 10.0;

/// How fast the display of the level falls back, in dB per second.
const LEVEL_FALL_DB_PER_SECOND: f32 = 20.0;

/// From which level on its display turns orange, and red close to the limiter, in dB of full
/// scale.
const LEVEL_WARM_DB: f32 = -6.0;
const LEVEL_HOT_DB: f32 = -1.0;

/// The width of the bars of the level and of the limiter, in logical pixels.
const BAR_WIDTH: f32 = 80.0;

/// From how much reduction on the display asks to lower the output level, in dB.
const LOWER_OUTPUT_DB: f32 = 3.0;

/// How often the window is drawn again, for the displays of the level and the limiter.
const METER_FRAME: Duration = Duration::from_millis(33);

/// The tabs of the window: where the correction comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    /// OPRA's presets.
    #[default]
    Opra,
    /// A measurement and a target.
    Curves,
}

/// What the window keeps between frames.
#[derive(Default)]
struct State {
    /// The open tab.
    tab: Tab,
    /// The path in the field of OPRA's file.
    path: String,
    /// The paths in the fields of the measurement and the target.
    measurement_path: String,
    target_path: String,
    /// The search in the field.
    query: String,
    /// The search and the database the results are for, and the results.
    results: Option<(String, Arc<Database>, Vec<String>)>,
    /// The chosen headphones.
    product: Option<String>,
    /// Whether the headphones of the saved preset were chosen again since the window opened, or
    /// there is nothing to choose.
    reselected: bool,
    /// Whether the list of headphones still has to scroll to the chosen ones.
    scroll_to_product: bool,
    /// The curve of the current correction.
    curve: Option<Curve>,
    /// What the limiter's display shows, in dB.
    meter: f32,
    /// What the display of the level shows, in dB of full scale; the floor before any signal.
    level: f32,
    /// Whether the about window is open.
    about: bool,
}

/// What a curve is drawn for.
#[derive(Debug, Clone, PartialEq)]
enum Shown {
    /// The preset with this ID.
    Preset(String),
    /// This design.
    Design(SavedDesign),
}

/// The curve of a correction, as the window draws it.
struct Curve {
    /// What it is for.
    shown: Shown,
    /// The frequencies, in Hz, and the response there, in dB.
    points: Vec<(f64, f64)>,
    /// By how much the correction is lowered, in dB.
    headroom_db: f64,
}

/// The window's state for nih-plug, saved with the project: its size.
#[must_use]
pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(SIZE.0, SIZE.1)
}

/// The editor of the plugin.
pub fn create(params: Arc<AudioRefParams>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    let read = |path: &std::sync::RwLock<String>| {
        path.read().unwrap_or_else(PoisonError::into_inner).clone()
    };
    let state = State {
        tab: if current_design(&params).is_some() {
            Tab::Curves
        } else {
            Tab::Opra
        },
        path: read(&params.opra_path),
        measurement_path: read(&params.measurement_path),
        target_path: read(&params.target_path),
        level: floor_db(),
        ..State::default()
    };
    let build_shared = shared.clone();
    let build_params = params.clone();
    create_egui_editor(
        params.editor_state.clone(),
        state,
        move |ctx, state| {
            theme::apply(ctx);
            // Reads the file again that was loaded last time, once per window.
            if !state.path.is_empty() && matches!(*lock(&build_shared.database), Load::None) {
                build_shared.execute(Task::Load(Some(PathBuf::from(&state.path))), &build_params);
            }
        },
        move |ctx, setter, state| {
            let lang = shared.lang;
            // Taken even with the displays off, so they don't start with what came meanwhile.
            let reduction = shared.take_reduction();
            let peak = shared.take_peak();
            if params.meters.load(Ordering::Relaxed) {
                // The displays hold the most, and fall back slowly.
                let dt = ctx.input(|input| input.stable_dt).min(0.1);
                state.meter = reduction
                    .max(state.meter - METER_FALL_DB_PER_SECOND * dt)
                    .clamp(0.0, METER_DB);
                state.level = gain_to_db(peak)
                    .max(state.level - LEVEL_FALL_DB_PER_SECOND * dt)
                    .max(floor_db());
                // The displays follow the signal.
                ctx.request_repaint_after(METER_FRAME);
            } else {
                // Off, the window is only drawn again when something changes in it.
                state.meter = 0.0;
                state.level = floor_db();
            }
            let margin = |top: i8, bottom: i8| egui::Margin {
                left: 14,
                right: 14,
                top,
                bottom,
            };
            egui::TopBottomPanel::top("header")
                .frame(
                    egui::Frame::new()
                        .fill(theme::BACKGROUND)
                        .inner_margin(margin(10, 6)),
                )
                .show_separator_line(false)
                .show(ctx, |ui| header(ui, lang, state));
            egui::TopBottomPanel::bottom("parameters")
                .frame(
                    egui::Frame::new()
                        .fill(theme::BACKGROUND)
                        .inner_margin(margin(4, 8)),
                )
                .show_separator_line(false)
                .show(ctx, |ui| {
                    parameters(ui, lang, &params, setter, state);
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(theme::note(Notice::NonCommercial.text(lang)));
                        ui.label(theme::note("·"));
                        ui.label(theme::note(Notice::MonitoringOnly.text(lang)));
                    });
                });
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(theme::BACKGROUND)
                        .inner_margin(margin(2, 6)),
                )
                .show(ctx, |ui| {
                    theme::card(ui, "", |ui| match state.tab {
                        Tab::Opra => database(ui, lang, state, &params, &shared),
                        Tab::Curves => curves(ui, lang, state, &params, &shared),
                    });
                    ui.add_space(8.0);
                    theme::card(ui, Text::Correction.text(lang), |ui| {
                        correction(ui, lang, state, &params, &shared);
                    });
                });
            about::window(ctx, lang, &mut state.about);
        },
    )
}

/// The name, which opens the about window, the notice of private use, and the button of the
/// about window.
fn header(ui: &mut egui::Ui, lang: Lang, state: &mut State) {
    ui.horizontal(|ui| {
        let name = egui::Label::new(
            egui::RichText::new("AudioRef")
                .size(22.0)
                .strong()
                .color(theme::ACCENT),
        )
        .sense(egui::Sense::click());
        if ui
            .add(name)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(About::Hint.text(lang))
            .clicked()
        {
            state.about = true;
        }
        ui.add_space(12.0);
        tabs(ui, lang, state);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(About::Button.text(lang))
                .on_hover_text(About::Hint.text(lang))
                .clicked()
            {
                state.about = !state.about;
            }
        });
    });
}

/// The tabs of where the correction comes from, as one control of two halves.
fn tabs(ui: &mut egui::Ui, lang: Lang, state: &mut State) {
    egui::Frame::new()
        .fill(theme::DEEP)
        .corner_radius(8)
        .inner_margin(3)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (tab, text) in [
                    (Tab::Opra, Text::SourceOpra),
                    (Tab::Curves, Text::SourceCurves),
                ] {
                    let selected = state.tab == tab;
                    let label = egui::RichText::new(text.text(lang)).color(if selected {
                        Color32::WHITE
                    } else {
                        theme::WEAK
                    });
                    if ui.selectable_label(selected, label).clicked() {
                        state.tab = tab;
                    }
                }
            });
        });
}

/// The search, the headphones and their presets; below, OPRA's file, folded away unless one is
/// set or failed.
fn database(
    ui: &mut egui::Ui,
    lang: Lang,
    state: &mut State,
    params: &AudioRefParams,
    shared: &Shared,
) {
    let load = lock(&shared.database).clone();
    // Without a file, or when it couldn't be loaded, the built-in presets.
    let (database, from_file, problem) = match load {
        Load::None => (Some(shared.bundled.clone()), false, None),
        Load::Loading => (None, false, None),
        Load::ReadFailed(reason) => (
            Some(shared.bundled.clone()),
            false,
            Some(format!("{} {reason}", Text::ReadFailed.text(lang))),
        ),
        Load::NotOpra => (
            Some(shared.bundled.clone()),
            false,
            Some(Text::NotOpra.text(lang).to_owned()),
        ),
        Load::Loaded(database) => (Some(database), true, None),
    };
    match database {
        Some(database) => browser(ui, lang, state, params, shared, &database, from_file),
        None => {
            ui.label(Text::Loading.text(lang));
        }
    }
    ui.add_space(2.0);
    egui::CollapsingHeader::new(Text::Database.text(lang))
        .id_salt("database")
        .default_open(!state.path.is_empty() || problem.is_some())
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let width = ui.available_width() - 70.0;
                ui.add(egui::TextEdit::singleline(&mut state.path).desired_width(width))
                    .on_hover_text(Hint::LoadDatabase.text(lang));
                if ui
                    .button(Text::Load.text(lang))
                    .on_hover_text(Hint::LoadDatabase.text(lang))
                    .clicked()
                {
                    // An empty path goes back to the built-in presets.
                    let path = state.path.trim().to_owned();
                    if path.is_empty() {
                        state.path.clear();
                    }
                    let task = Task::Load((!path.is_empty()).then(|| PathBuf::from(&path)));
                    *params
                        .opra_path
                        .write()
                        .unwrap_or_else(PoisonError::into_inner) = path;
                    shared.execute(task, params);
                }
            });
            if let Some(problem) = problem {
                ui.colored_label(theme::HOT, problem);
            }
            ui.add(egui::Label::new(theme::note(Text::DatabaseHint.text(lang))).wrap());
        });
}

/// The search with what it searches, and the headphones it found beside their presets.
fn browser(
    ui: &mut egui::Ui,
    lang: Lang,
    state: &mut State,
    params: &AudioRefParams,
    shared: &Shared,
    database: &Arc<Database>,
    from_file: bool,
) {
    if !state.reselected {
        reselect(state, database, params);
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .hint_text(Text::Search.text(lang))
                .desired_width(300.0),
        )
        .on_hover_text(Hint::Search.text(lang));
        let presets = database.presets().len();
        let headphones = database.products().len();
        let status = if from_file {
            Message::Loaded {
                presets,
                headphones,
            }
        } else {
            Message::BuiltIn {
                presets,
                headphones,
            }
        };
        ui.label(theme::note(status.text(lang)));
        if database.skipped() > 0 {
            ui.label(theme::note(Message::Skipped(database.skipped()).text(lang)));
        }
    });
    let outdated = state.results.as_ref().is_none_or(|(query, searched, _)| {
        *query != state.query || !Arc::ptr_eq(searched, database)
    });
    if outdated {
        let found = database
            .search(&state.query, RESULTS)
            .into_iter()
            .map(str::to_owned)
            .collect();
        state.results = Some((state.query.clone(), database.clone(), found));
    }
    ui.columns(2, |columns| {
        if let [left, right] = columns {
            list_frame(left, |ui| headphones(ui, lang, state, database));
            list_frame(right, |ui| {
                presets(ui, lang, state, database, params, shared);
            });
        }
    });
}

/// The deep background of a list.
fn list_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::DEEP)
        .corner_radius(6)
        .inner_margin(4)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(LIST_HEIGHT);
            add(ui);
        });
}

/// Chooses the headphones of the saved preset again when the window opens, so the browser shows
/// the preset selected; searches for their name if the search wouldn't list them. Tries again
/// with each database until one has the preset.
fn reselect(state: &mut State, database: &Database, params: &AudioRefParams) {
    if state.product.is_some() || current_design(params).is_some() {
        state.reselected = true;
        return;
    }
    let Some(saved) = current(params) else {
        state.reselected = true;
        return;
    };
    let Some(product) = database.product_of(&saved.preset_id) else {
        return;
    };
    if !database.search(&state.query, RESULTS).contains(&product) {
        state.query = database.headphones(product);
    }
    state.product = Some(product.to_owned());
    state.scroll_to_product = true;
    state.reselected = true;
}

/// The headphones the search found.
fn headphones(ui: &mut egui::Ui, lang: Lang, state: &mut State, database: &Database) {
    let Some((_, _, found)) = &state.results else {
        return;
    };
    if found.is_empty() {
        ui.label(Text::NoMatches.text(lang));
    }
    let mut chosen = None;
    egui::ScrollArea::vertical()
        .id_salt("headphones")
        .max_height(LIST_HEIGHT)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.spacing_mut().interact_size.y = 18.0;
            for id in found {
                let selected = state.product.as_deref() == Some(id.as_str());
                let response = ui.selectable_label(selected, database.headphones(id));
                if selected && state.scroll_to_product {
                    response.scroll_to_me(Some(egui::Align::Center));
                    state.scroll_to_product = false;
                }
                if response.clicked() {
                    chosen = Some(id.clone());
                }
            }
        });
    if chosen.is_some() {
        state.product = chosen;
    }
}

/// The presets of the chosen headphones; a click makes one the correction.
fn presets(
    ui: &mut egui::Ui,
    lang: Lang,
    state: &State,
    database: &Database,
    params: &AudioRefParams,
    shared: &Shared,
) {
    let Some(product) = &state.product else {
        ui.add(egui::Label::new(theme::note(Hint::PickHeadphones.text(lang))).wrap());
        return;
    };
    ui.label(theme::title_text(Text::Presets.text(lang)));
    let current = current_design(params)
        .is_none()
        .then(|| current(params).map(|saved| saved.preset_id))
        .flatten();
    egui::ScrollArea::vertical()
        .id_salt("presets")
        .max_height(LIST_HEIGHT - 20.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.spacing_mut().interact_size.y = 18.0;
            for (id, preset) in database.presets_of(product) {
                let label = if preset.details.is_empty() {
                    preset.author.clone()
                } else {
                    format!("{}: {}", preset.author, preset.details)
                };
                let selected = current.as_deref() == Some(id);
                if ui
                    .selectable_label(selected, label)
                    .on_hover_text(Hint::Preset.text(lang))
                    .clicked()
                    && !selected
                {
                    set_correction(params, shared, SavedCorrection::from_preset(database, id));
                }
            }
        });
}

/// The files of the measurement and the target, and the button that makes them the correction.
fn curves(
    ui: &mut egui::Ui,
    lang: Lang,
    state: &mut State,
    params: &AudioRefParams,
    shared: &Shared,
) {
    ui.add(egui::Label::new(egui::RichText::new(Text::CurvesHint.text(lang)).small()).wrap());
    let rows = [
        (
            Which::Measurement,
            Text::Measurement,
            &mut state.measurement_path,
            &params.measurement_path,
        ),
        (
            Which::Target,
            Text::Target,
            &mut state.target_path,
            &params.target_path,
        ),
    ];
    for (which, label, path, saved_path) in rows {
        ui.horizontal(|ui| {
            ui.add_sized([90.0, 20.0], egui::Label::new(label.text(lang)));
            ui.add(egui::TextEdit::singleline(path).desired_width(400.0))
                .on_hover_text(Hint::LoadCurve.text(lang));
            if ui
                .button(Text::Load.text(lang))
                .on_hover_text(Hint::LoadCurve.text(lang))
                .clicked()
            {
                let trimmed = path.trim().to_owned();
                saved_path
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone_from(&trimmed);
                if !trimmed.is_empty() {
                    shared.execute(Task::LoadCurve(which, PathBuf::from(trimmed)), params);
                }
            }
            // Beside the field, so the fields below don't move.
            curve_status(ui, lang, &lock(shared.curve(which)));
        });
    }
    let measurement = lock(&shared.measurement).clone();
    let target = lock(&shared.target).clone();
    let ready = match (&measurement, &target) {
        (CurveLoad::Loaded(m, measurement), CurveLoad::Loaded(t, target)) => {
            Some(((m, measurement), (t, target)))
        }
        _ => None,
    };
    let button = egui::Button::new(Text::UseCurves.text(lang));
    if ui
        .add_enabled(ready.is_some(), button)
        .on_hover_text(Hint::UseCurves.text(lang))
        .on_disabled_hover_text(Hint::LoadBoth.text(lang))
        .clicked()
        && let Some(((m, measurement), (t, target))) = ready
    {
        // The settings stay those of the design before, if there was one.
        let settings = current_design(params).map_or_else(Settings::default, |d| d.settings);
        let design = SavedDesign::new((m, measurement), (t, target), settings);
        set_design(params, shared, Some(design));
    }
}

/// Whether the file of a curve was read, and why not.
fn curve_status(ui: &mut egui::Ui, lang: Lang, load: &CurveLoad) {
    let (text, color) = match load {
        CurveLoad::None => return,
        CurveLoad::Loaded(name, curve) => {
            let points = CurveMessage::Points(curve.points().len()).text(lang);
            let text = if curve.skipped() > 0 {
                let skipped = Message::Skipped(curve.skipped()).text(lang);
                format!("{name}: {points}, {skipped}")
            } else {
                format!("{name}: {points}")
            };
            (text, Color32::GRAY)
        }
        CurveLoad::ReadFailed(reason) => (
            format!("{} {reason}", Text::ReadFailed.text(lang)),
            Color32::LIGHT_RED,
        ),
        CurveLoad::Invalid(error) => {
            let text = match *error {
                ParseError::NoPoints => Text::NotCurve.text(lang).to_owned(),
                ParseError::TooManyPoints => Text::TooManyPoints.text(lang).to_owned(),
                ParseError::TooNarrow { from_hz, to_hz } => {
                    CurveMessage::TooNarrow { from_hz, to_hz }.text(lang)
                }
            };
            (text, Color32::LIGHT_RED)
        }
    };
    ui.add(egui::Label::new(egui::RichText::new(text).small().color(color)).wrap());
}

/// The current correction: where it comes from, its settings, its headroom, its curve, and the
/// button to turn it off.
fn correction(
    ui: &mut egui::Ui,
    lang: Lang,
    state: &mut State,
    params: &AudioRefParams,
    shared: &Shared,
) {
    if let Some(design) = current_design(params) {
        designed(ui, lang, state, params, shared, design);
        return;
    }
    let Some(saved) = current(params) else {
        ui.label(theme::note(Text::NoCorrection.text(lang)));
        state.curve = None;
        // The grid with a flat line, so the window keeps its shape.
        let flat: Vec<(f64, f64)> = grid().map(|frequency| (frequency, 0.0)).collect();
        curve(ui, lang, &flat, None);
        return;
    };
    let shown = Shown::Preset(saved.preset_id.clone());
    if state
        .curve
        .as_ref()
        .is_none_or(|curve| curve.shown != shown)
    {
        state.curve = saved.eq().map(|eq| {
            // The curve as at 48 kHz: at other sample rates it differs only near the top.
            let cascade = Cascade::new(&eq, 48_000.0);
            let points = grid()
                .map(|frequency| (frequency, cascade.response_db(frequency)))
                .collect();
            Curve {
                shown,
                points,
                headroom_db: cascade.headroom_db(),
            }
        });
    }
    let attribution = Message::Attribution {
        author: &saved.author,
        details: &saved.details,
    };
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(Text::TurnOff.text(lang))
            .on_hover_text(Hint::TurnOff.text(lang))
            .clicked()
        {
            set_correction(params, shared, None);
        }
        ui.label(egui::RichText::new(&saved.headphones).strong());
        ui.label(theme::note(attribution.text(lang)));
        ui.hyperlink_to(egui::RichText::new(OPRA_URL).small(), OPRA_URL);
    });
    show_curve(ui, lang, state.curve.as_ref());
}

/// A correction designed from a measurement and a target: their names, the settings, the curve.
fn designed(
    ui: &mut egui::Ui,
    lang: Lang,
    state: &mut State,
    params: &AudioRefParams,
    shared: &Shared,
    mut design: SavedDesign,
) {
    ui.horizontal(|ui| {
        let names = CurveMessage::Designed {
            measurement: &design.measurement,
            target: &design.target,
        };
        ui.label(egui::RichText::new(names.text(lang)).strong());
        if ui
            .button(Text::TurnOff.text(lang))
            .on_hover_text(Hint::TurnOff.text(lang))
            .clicked()
        {
            set_design(params, shared, None);
        }
    });
    // The curve follows a slider at once; the filter, which takes longer, once it is let go.
    let mut changed = false;
    let mut released = false;
    let settings = &mut design.settings;
    let sliders = [
        (
            &mut settings.tilt_db_per_octave,
            TILT_DB_PER_OCTAVE,
            Text::Tilt,
            Hint::Tilt,
            Text::DbPerOctave.text(lang),
            0.1,
        ),
        (
            &mut settings.bass_db,
            BASS_DB,
            Text::Bass,
            Hint::Bass,
            " dB",
            0.5,
        ),
        (
            &mut settings.max_boost_db,
            MAX_BOOST_DB,
            Text::MaxBoost,
            Hint::MaxBoost,
            " dB",
            0.5,
        ),
    ];
    // A column each, so the row fits in every language.
    ui.columns(sliders.len(), |columns| {
        for ((value, range, name, hint, unit, step), ui) in sliders.into_iter().zip(columns) {
            ui.horizontal(|ui| {
                ui.label(name.text(lang)).on_hover_text(hint.text(lang));
                ui.spacing_mut().slider_width =
                    (ui.available_width() - SLIDER_VALUE_WIDTH).max(SLIDER_MIN_WIDTH);
                let response = ui
                    .add(egui::Slider::new(value, range).suffix(unit).step_by(step))
                    .on_hover_text(hint.text(lang));
                changed |= response.changed();
                released |= response.drag_stopped() || (response.changed() && !response.dragged());
            });
        }
    });
    if changed {
        *params
            .design
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(design.clone());
    }
    if released {
        shared.execute(Task::Design, params);
    }
    let shown = Shown::Design(design);
    if state
        .curve
        .as_ref()
        .is_none_or(|curve| curve.shown != shown)
    {
        state.curve = match &shown {
            Shown::Design(design) => design.correction().map(|correction| {
                let headroom_db = correction.headroom_db();
                let points = grid()
                    .map(|frequency| (frequency, correction.response_db(frequency) - headroom_db))
                    .collect();
                Curve {
                    shown: shown.clone(),
                    points,
                    headroom_db,
                }
            }),
            Shown::Preset(_) => None,
        };
    }
    show_curve(ui, lang, state.curve.as_ref());
}

/// The curve of a correction, with its headroom.
fn show_curve(ui: &mut egui::Ui, lang: Lang, shown: Option<&Curve>) {
    if let Some(shown) = shown {
        let headroom =
            (shown.headroom_db > 0.05).then(|| Message::Headroom(shown.headroom_db).text(lang));
        curve(ui, lang, &shown.points, headroom);
    }
}

/// The saved design, if the correction is one.
fn current_design(params: &AudioRefParams) -> Option<SavedDesign> {
    params
        .design
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// The saved correction.
fn current(params: &AudioRefParams) -> Option<SavedCorrection> {
    params
        .correction
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Saves a preset as the correction, or none, and has the background design it. A design, if
/// there was one, goes: the preset takes its place.
fn set_correction(params: &AudioRefParams, shared: &Shared, correction: Option<SavedCorrection>) {
    *params
        .correction
        .write()
        .unwrap_or_else(PoisonError::into_inner) = correction;
    *params
        .design
        .write()
        .unwrap_or_else(PoisonError::into_inner) = None;
    shared.execute(Task::Design, params);
}

/// Saves a design as the correction, or none, and has the background design it. The preset, if
/// there was one, goes: the design takes its place.
fn set_design(params: &AudioRefParams, shared: &Shared, design: Option<SavedDesign>) {
    *params
        .design
        .write()
        .unwrap_or_else(PoisonError::into_inner) = design;
    *params
        .correction
        .write()
        .unwrap_or_else(PoisonError::into_inner) = None;
    shared.execute(Task::Design, params);
}

/// The response of the correction over the frequencies, on a logarithmic axis from 20 Hz to
/// 20 kHz, filled to 0 dB; under the pointer, the frequency and the response there.
fn curve(ui: &mut egui::Ui, lang: Lang, points: &[(f64, f64)], headroom: Option<String>) {
    let height = (ui.available_height() - 4.0).clamp(CURVE_MIN_HEIGHT, CURVE_MAX_HEIGHT);
    let (response, painter) = ui.allocate_painter(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let rect: Rect = response.rect;
    let hovered = response.hover_pos();
    let hint = if headroom.is_some() {
        format!(
            "{}\n\n{}",
            Hint::Curve.text(lang),
            Hint::Headroom.text(lang)
        )
    } else {
        Hint::Curve.text(lang).to_owned()
    };
    response.on_hover_text(hint);
    let to_x = |frequency: f64| {
        #[expect(clippy::cast_possible_truncation, reason = "a position on the screen")]
        let share = ((frequency / 20.0).log10() / 3.0) as f32;
        rect.left() + share * rect.width()
    };
    let to_y = |db: f64| {
        #[expect(clippy::cast_possible_truncation, reason = "a position on the screen")]
        let db = (db as f32).clamp(CURVE_DB.1, CURVE_DB.0);
        rect.top() + (CURVE_DB.0 - db) / (CURVE_DB.0 - CURVE_DB.1) * rect.height()
    };
    painter.rect_filled(rect, 6.0, theme::DEEP);
    draw_grid(&painter, rect, &to_x, &to_y);
    let line: Vec<Pos2> = points
        .iter()
        .map(|&(frequency, db)| Pos2::new(to_x(frequency), to_y(db)))
        .collect();
    // The area between the curve and 0 dB, in strips.
    let zero = to_y(0.0);
    let mut area = egui::Mesh::default();
    for pair in line.windows(2) {
        if let [a, b] = pair {
            let index = u32::try_from(area.vertices.len()).unwrap_or(u32::MAX);
            for point in [*a, Pos2::new(a.x, zero), *b, Pos2::new(b.x, zero)] {
                area.colored_vertex(point, theme::ACCENT_DIM);
            }
            area.add_triangle(index, index + 1, index + 2);
            area.add_triangle(index + 1, index + 3, index + 2);
        }
    }
    painter.add(area);
    painter.add(egui::Shape::line(line, Stroke::new(2.0_f32, theme::ACCENT)));
    // Beside the label of 0 dB, which the curve never goes above.
    if let Some(headroom) = headroom {
        painter.text(
            Pos2::new(rect.left() + 44.0, to_y(0.0) - 2.0),
            egui::Align2::LEFT_BOTTOM,
            headroom,
            egui::FontId::proportional(11.0),
            theme::WEAK,
        );
    }
    if let Some(at) = hovered {
        hover_point(&painter, rect, at.x, points, lang);
    }
}

/// The grid of the curve: lines at 2, 5 and 10 times each decade, and every 6 dB, with labels.
fn draw_grid(
    painter: &egui::Painter,
    rect: Rect,
    to_x: &dyn Fn(f64) -> f32,
    to_y: &dyn Fn(f64) -> f32,
) {
    let grid = Stroke::new(1.0_f32, theme::GRID);
    let label = |position: Pos2, align: egui::Align2, text: String| {
        painter.text(
            position,
            align,
            text,
            egui::FontId::proportional(10.0),
            theme::WEAK,
        );
    };
    // Each decade from 20 Hz, its labels at 100 Hz, 1 kHz and 10 kHz.
    for decade in [10.0, 100.0, 1000.0, 10_000.0] {
        for (step, labelled) in [(2.0, false), (5.0, false), (10.0, true)] {
            let frequency: f64 = decade * step;
            if !(20.0..=20_000.0).contains(&frequency) {
                continue;
            }
            let x = to_x(frequency);
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                grid,
            );
            if labelled && frequency < 20_000.0 {
                let text = if frequency < 1000.0 {
                    format!("{frequency} Hz")
                } else {
                    format!("{} kHz", frequency / 1000.0)
                };
                label(
                    Pos2::new(x + 4.0, rect.bottom() - 3.0),
                    egui::Align2::LEFT_BOTTOM,
                    text,
                );
            }
        }
    }
    for db in [0.0, -6.0, -12.0, -18.0] {
        let y = to_y(db);
        let stroke = if db == 0.0 {
            Stroke::new(1.0_f32, Color32::from_gray(90))
        } else {
            grid
        };
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            stroke,
        );
        label(
            Pos2::new(rect.left() + 4.0, y - 2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{db} dB"),
        );
    }
}

/// Under the pointer: a line at its frequency, a dot on the curve, and their values.
fn hover_point(painter: &egui::Painter, rect: Rect, x: f32, points: &[(f64, f64)], lang: Lang) {
    let share = f64::from((x - rect.left()) / rect.width());
    let frequency = 20.0 * 10.0_f64.powf(3.0 * share.clamp(0.0, 1.0));
    // The nearest point of the grid, which is dense enough.
    let Some(&(at, db)) = points.iter().min_by(|a, b| {
        (a.0 / frequency)
            .ln()
            .abs()
            .total_cmp(&(b.0 / frequency).ln().abs())
    }) else {
        return;
    };
    #[expect(clippy::cast_possible_truncation, reason = "a position on the screen")]
    let (x, y) = {
        let share = ((at / 20.0).log10() / 3.0) as f32;
        let db = (db as f32).clamp(CURVE_DB.1, CURVE_DB.0);
        (
            rect.left() + share * rect.width(),
            rect.top() + (CURVE_DB.0 - db) / (CURVE_DB.0 - CURVE_DB.1) * rect.height(),
        )
    };
    painter.line_segment(
        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
        Stroke::new(1.0_f32, Color32::from_gray(110)),
    );
    painter.circle_filled(Pos2::new(x, y), 4.0, theme::ACCENT);
    let text = Message::CurvePoint {
        frequency_hz: at,
        db,
    }
    .text(lang);
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.top() + 6.0),
        egui::Align2::RIGHT_TOP,
        text,
        egui::FontId::proportional(12.0),
        theme::TEXT,
    );
}

/// The parameters: the output level, the bypass and the render guard; the routing of the
/// channels, the crossfeed, and the limiter with its display.
fn parameters(
    ui: &mut egui::Ui,
    lang: Lang,
    params: &AudioRefParams,
    setter: &ParamSetter,
    state: &State,
) {
    let on = params.meters.load(Ordering::Relaxed);
    let mut clicked = false;
    ui.columns(3, |columns| {
        let [output, stereo, protection] = columns else {
            return;
        };
        theme::card_of_height(output, params.output.name(), CONTROLS_HEIGHT, |ui| {
            ui.add(
                widgets::ParamSlider::for_param(&params.output, setter)
                    .with_width(ui.available_width() - 64.0),
            )
            .on_hover_text(Hint::Output.text(lang));
            ui.horizontal(|ui| clicked |= level(ui, lang, on, state.level));
            toggle(ui, &params.bypass, setter, Hint::Bypass.text(lang));
        });
        theme::card_of_height(stereo, Text::Stereo.text(lang), CONTROLS_HEIGHT, |ui| {
            egui::Grid::new("stereo")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    let channels =
                        Channels::ALL.map(|channels| (channels.index(), channels.text(lang)));
                    choice(
                        ui,
                        &params.channels,
                        setter,
                        &channels,
                        Hint::Channels.text(lang),
                    );
                    ui.end_row();
                    let crossfeed =
                        Crossfeed::ALL.map(|crossfeed| (crossfeed.index(), crossfeed.text(lang)));
                    choice(
                        ui,
                        &params.crossfeed,
                        setter,
                        &crossfeed,
                        Hint::Crossfeed.text(lang),
                    );
                    ui.end_row();
                });
        });
        theme::card_of_height(
            protection,
            Text::Protection.text(lang),
            CONTROLS_HEIGHT,
            |ui| {
                ui.horizontal(|ui| {
                    toggle(ui, &params.limiter, setter, Hint::Limiter.text(lang));
                    clicked |= reduction(ui, lang, on, state.meter);
                });
                toggle(
                    ui,
                    &params.bypass_on_export,
                    setter,
                    Hint::BypassOnExport.text(lang),
                );
                // Always a line, so the cards don't change their height.
                let lower = on && state.meter >= LOWER_OUTPUT_DB;
                let text = if lower {
                    Hint::LowerOutput.text(lang)
                } else {
                    ""
                };
                ui.label(egui::RichText::new(text).small().color(theme::WARM));
            },
        );
    });
    if clicked {
        params.meters.store(!on, Ordering::Relaxed);
    }
}

/// The lowest level the display shows, in dB of full scale.
fn floor_db() -> f32 {
    #[expect(clippy::cast_possible_truncation, reason = "a level for the display")]
    let floor = LEVEL_FLOOR_DB as f32;
    floor
}

/// A bar of a display, filled to a share from 0 to 1, with a hint; crossed out if the displays
/// are off. Whether it was clicked, which turns the displays on or off.
fn bar(ui: &mut egui::Ui, lang: Lang, on: bool, share: f32, color: Color32, hint: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(BAR_WIDTH, 8.0), egui::Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, theme::DEEP);
    if on {
        let mut filled = rect;
        filled.set_width(rect.width() * share.clamp(0.0, 1.0));
        if filled.width() > 0.5 {
            painter.rect_filled(filled, 4.0, color);
        }
    } else {
        let stroke = egui::Stroke::new(1.5_f32, theme::WEAK);
        painter.line_segment([rect.left_bottom(), rect.right_top()], stroke);
        painter.line_segment([rect.left_top(), rect.right_bottom()], stroke);
    }
    let click = if on { Hint::MetersOn } else { Hint::MetersOff };
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!("{hint}\n\n{}", click.text(lang)))
        .clicked()
}

/// The space of a display's number, empty while the displays are off.
fn number(ui: &mut egui::Ui, on: bool, text: String, color: Color32) {
    let text = if on { text } else { String::new() };
    ui.add_sized(
        [44.0, 14.0],
        egui::Label::new(egui::RichText::new(text).small().color(color)),
    );
}

/// The peak level of the output: a bar from [`LEVEL_FLOOR_DB`] to 0 dB of full scale, and the
/// number.
fn level(ui: &mut egui::Ui, lang: Lang, on: bool, db: f32) -> bool {
    let floor = floor_db();
    let hint = Hint::Level.text(lang);
    ui.label(Text::Level.text(lang)).on_hover_text(hint);
    let color = if db >= LEVEL_HOT_DB {
        theme::HOT
    } else if db >= LEVEL_WARM_DB {
        theme::WARM
    } else {
        theme::GOOD
    };
    let clicked = bar(ui, lang, on, (db - floor) / -floor, color, hint);
    let text = Message::Level(f64::from(db)).text(lang);
    number(ui, on, text, theme::WEAK);
    clicked
}

/// How much the limiter lowers the signal: a bar from 0 to [`METER_DB`], the number, and from
/// [`LOWER_OUTPUT_DB`] on the hint to lower the output level.
fn reduction(ui: &mut egui::Ui, lang: Lang, on: bool, db: f32) -> bool {
    let clicked = bar(
        ui,
        lang,
        on,
        db / METER_DB,
        theme::WARM,
        Hint::Limiter.text(lang),
    );
    let text = Message::Reduction(f64::from(db)).text(lang);
    let color = if db > 0.05 { theme::WARM } else { theme::WEAK };
    number(ui, on, text, color);
    clicked
}

/// A list to choose a value of a parameter from, each value with its name; the host learns of a
/// choice as a gesture.
fn choice(
    ui: &mut egui::Ui,
    param: &IntParam,
    setter: &ParamSetter,
    names: &[(i32, &str)],
    hint: &str,
) {
    let current = param.value();
    let selected = names
        .iter()
        .find(|(index, _)| *index == current)
        .map_or("", |(_, name)| *name);
    ui.label(param.name()).on_hover_text(hint);
    egui::ComboBox::from_id_salt(param.name())
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for &(index, name) in names {
                if ui.selectable_label(index == current, name).clicked() && index != current {
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, index);
                    setter.end_set_parameter(param);
                }
            }
        })
        .response
        .on_hover_text(hint);
}

/// A checkbox for a boolean parameter, with its hint; the host learns of a change as a gesture.
fn toggle(ui: &mut egui::Ui, param: &BoolParam, setter: &ParamSetter, hint: &str) {
    let mut value = param.value();
    if ui
        .checkbox(&mut value, param.name())
        .on_hover_text(hint)
        .changed()
    {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, value);
        setter.end_set_parameter(param);
    }
}
