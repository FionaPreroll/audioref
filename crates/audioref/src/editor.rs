//! The window, drawn with egui, in two tabs: OPRA's file or the built-in presets, the search for
//! headphones and their presets; or the files of a measurement and a target. Below them the
//! correction with its curve and, for a measurement and a target, its settings; the parameters
//! and the notices.
//!
//! Only reading and showing: the files are read and the correction designed in the background,
//! see [`crate::background`].

use std::path::PathBuf;
use std::sync::{Arc, PoisonError};

use audioref_core::autoeq::ParseError;
use audioref_core::cascade::{Cascade, grid};
use audioref_core::design::{BASS_DB, MAX_BOOST_DB, Settings, TILT_DB_PER_OCTAVE};
use audioref_core::i18n::{CurveMessage, Lang, Message, Notice, Text};
use audioref_core::opra::Database;
use audioref_core::state::{SavedCorrection, SavedDesign};
use audioref_core::stereo::{Channels, Crossfeed};
use nih_plug::prelude::{BoolParam, Editor, IntParam, Param, ParamSetter};
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use nih_plug_egui::{EguiState, create_egui_editor, widgets};

use crate::AudioRefParams;
use crate::background::{CurveLoad, Load, Shared, Task, Which, lock};

/// The size of the window, in logical pixels.
pub const SIZE: (u32, u32) = (760, 680);

/// The most headphones the search lists.
const RESULTS: usize = 200;

/// OPRA's repository, linked under the preset's author, as OPRA asks.
const OPRA_URL: &str = "https://github.com/opra-project/OPRA";

/// The range of the curve, in dB: a correction boosts nothing, see the headroom.
const CURVE_DB: (f32, f32) = (6.0, -24.0);

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
    /// The curve of the current correction.
    curve: Option<Curve>,
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
        ..State::default()
    };
    let build_shared = shared.clone();
    let build_params = params.clone();
    create_egui_editor(
        params.editor_state.clone(),
        state,
        move |_, state| {
            // Reads the file again that was loaded last time, once per window.
            if !state.path.is_empty() && matches!(*lock(&build_shared.database), Load::None) {
                build_shared.execute(Task::Load(Some(PathBuf::from(&state.path))), &build_params);
            }
        },
        move |ctx, setter, state| {
            let lang = shared.lang;
            egui::TopBottomPanel::top("notice").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("AudioRef");
                    ui.label(Notice::NonCommercial.text(lang));
                });
            });
            egui::TopBottomPanel::bottom("parameters").show(ctx, |ui| {
                parameters(ui, lang, &params, setter);
                ui.label(egui::RichText::new(Notice::MonitoringOnly.text(lang)).small());
            });
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.tab, Tab::Opra, Text::SourceOpra.text(lang));
                    ui.selectable_value(&mut state.tab, Tab::Curves, Text::SourceCurves.text(lang));
                });
                ui.separator();
                match state.tab {
                    Tab::Opra => database(ui, lang, state, &params, &shared),
                    Tab::Curves => curves(ui, lang, state, &params, &shared),
                }
                ui.separator();
                correction(ui, lang, state, &params, &shared);
            });
        },
    )
}

/// The file or the built-in presets, the status, the search and the presets.
fn database(
    ui: &mut egui::Ui,
    lang: Lang,
    state: &mut State,
    params: &AudioRefParams,
    shared: &Shared,
) {
    ui.strong(Text::Database.text(lang));
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut state.path).desired_width(560.0));
        if ui.button(Text::Load.text(lang)).clicked() {
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
    ui.label(egui::RichText::new(Text::DatabaseHint.text(lang)).small());
    let load = lock(&shared.database).clone();
    // Without a file, or when it couldn't be loaded, the built-in presets.
    let (database, from_file) = match load {
        Load::None => (shared.bundled.clone(), false),
        Load::Loading => {
            ui.label(Text::Loading.text(lang));
            return;
        }
        Load::ReadFailed(reason) => {
            ui.colored_label(
                Color32::LIGHT_RED,
                format!("{} {reason}", Text::ReadFailed.text(lang)),
            );
            (shared.bundled.clone(), false)
        }
        Load::NotOpra => {
            ui.colored_label(Color32::LIGHT_RED, Text::NotOpra.text(lang));
            (shared.bundled.clone(), false)
        }
        Load::Loaded(database) => (database, true),
    };
    ui.horizontal(|ui| {
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
        ui.label(status.text(lang));
        if database.skipped() > 0 {
            ui.label(Message::Skipped(database.skipped()).text(lang));
        }
    });
    ui.add(egui::TextEdit::singleline(&mut state.query).hint_text(Text::Search.text(lang)));
    let outdated = state.results.as_ref().is_none_or(|(query, searched, _)| {
        *query != state.query || !Arc::ptr_eq(searched, &database)
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
            headphones(left, lang, state, &database);
            presets(right, lang, state, &database, params, shared);
        }
    });
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
        .max_height(220.0)
        .show(ui, |ui| {
            for id in found {
                let selected = state.product.as_deref() == Some(id.as_str());
                if ui
                    .selectable_label(selected, database.headphones(id))
                    .clicked()
                {
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
        return;
    };
    ui.strong(Text::Presets.text(lang));
    let current = current_design(params)
        .is_none()
        .then(|| current(params).map(|saved| saved.preset_id))
        .flatten();
    egui::ScrollArea::vertical()
        .id_salt("presets")
        .max_height(200.0)
        .show(ui, |ui| {
            for (id, preset) in database.presets_of(product) {
                let label = if preset.details.is_empty() {
                    preset.author.clone()
                } else {
                    format!("{}: {}", preset.author, preset.details)
                };
                let selected = current.as_deref() == Some(id);
                if ui.selectable_label(selected, label).clicked() && !selected {
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
            ui.add(egui::TextEdit::singleline(path).desired_width(400.0));
            if ui.button(Text::Load.text(lang)).clicked() {
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
    if ui.add_enabled(ready.is_some(), button).clicked()
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
    ui.strong(Text::Correction.text(lang));
    if let Some(design) = current_design(params) {
        designed(ui, lang, state, params, shared, design);
        return;
    }
    let Some(saved) = current(params) else {
        ui.label(Text::NoCorrection.text(lang));
        state.curve = None;
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
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&saved.headphones).strong());
        if ui.button(Text::TurnOff.text(lang)).clicked() {
            set_correction(params, shared, None);
        }
    });
    let attribution = Message::Attribution {
        author: &saved.author,
        details: &saved.details,
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(attribution.text(lang)).small());
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
        if ui.button(Text::TurnOff.text(lang)).clicked() {
            set_design(params, shared, None);
        }
    });
    // The curve follows a slider at once; the filter, which takes longer, once it is let go.
    let mut changed = false;
    let mut released = false;
    ui.horizontal(|ui| {
        let settings = &mut design.settings;
        let sliders = [
            (
                &mut settings.tilt_db_per_octave,
                TILT_DB_PER_OCTAVE,
                Text::Tilt,
                Text::DbPerOctave.text(lang),
                0.1,
            ),
            (&mut settings.bass_db, BASS_DB, Text::Bass, " dB", 0.5),
            (
                &mut settings.max_boost_db,
                MAX_BOOST_DB,
                Text::MaxBoost,
                " dB",
                0.5,
            ),
        ];
        for (value, range, name, unit, step) in sliders {
            ui.label(name.text(lang));
            let response = ui.add(egui::Slider::new(value, range).suffix(unit).step_by(step));
            changed |= response.changed();
            released |= response.drag_stopped() || (response.changed() && !response.dragged());
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

/// The headroom and the curve of a correction.
fn show_curve(ui: &mut egui::Ui, lang: Lang, shown: Option<&Curve>) {
    if let Some(shown) = shown {
        if shown.headroom_db > 0.05 {
            ui.label(Message::Headroom(shown.headroom_db).text(lang));
        }
        curve(ui, &shown.points);
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

/// The response of the correction over the frequencies, on a logarithmic axis.
fn curve(ui: &mut egui::Ui, points: &[(f64, f64)]) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 180.0), egui::Sense::hover());
    let rect: Rect = response.rect;
    let grid_stroke = Stroke::new(1.0_f32, Color32::from_gray(70));
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
    painter.rect_filled(rect, 4.0, Color32::from_gray(24));
    for frequency in [100.0, 1000.0, 10_000.0] {
        let x = to_x(frequency);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            grid_stroke,
        );
        let label = if frequency < 1000.0 {
            "100 Hz".to_owned()
        } else {
            format!("{} kHz", frequency / 1000.0)
        };
        painter.text(
            Pos2::new(x + 3.0, rect.bottom() - 3.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
    }
    for db in [0.0, -6.0, -12.0, -18.0] {
        let y = to_y(db);
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            grid_stroke,
        );
        painter.text(
            Pos2::new(rect.left() + 3.0, y - 2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{db} dB"),
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
    }
    let line: Vec<Pos2> = points
        .iter()
        .map(|&(frequency, db)| Pos2::new(to_x(frequency), to_y(db)))
        .collect();
    painter.add(egui::Shape::line(
        line,
        Stroke::new(2.0_f32, Color32::from_rgb(110, 190, 255)),
    ));
}

/// The parameters: the output level, the bypass and the render guard; the routing of the
/// channels and the crossfeed.
fn parameters(ui: &mut egui::Ui, lang: Lang, params: &AudioRefParams, setter: &ParamSetter) {
    ui.horizontal(|ui| {
        ui.label(params.output.name());
        ui.add(widgets::ParamSlider::for_param(&params.output, setter));
        toggle(ui, &params.bypass, setter);
        toggle(ui, &params.bypass_on_export, setter);
    });
    ui.horizontal(|ui| {
        let channels = Channels::ALL.map(|channels| (channels.index(), channels.text(lang)));
        choice(ui, &params.channels, setter, &channels);
        ui.add_space(12.0);
        let crossfeed = Crossfeed::ALL.map(|crossfeed| (crossfeed.index(), crossfeed.text(lang)));
        choice(ui, &params.crossfeed, setter, &crossfeed);
    });
}

/// A list to choose a value of a parameter from, each value with its name; the host learns of a
/// choice as a gesture.
fn choice(ui: &mut egui::Ui, param: &IntParam, setter: &ParamSetter, names: &[(i32, &str)]) {
    let current = param.value();
    let selected = names
        .iter()
        .find(|(index, _)| *index == current)
        .map_or("", |(_, name)| *name);
    ui.label(param.name());
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
        });
}

/// A checkbox for a boolean parameter, which the host learns of as a gesture.
fn toggle(ui: &mut egui::Ui, param: &BoolParam, setter: &ParamSetter) {
    let mut value = param.value();
    if ui.checkbox(&mut value, param.name()).changed() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, value);
        setter.end_set_parameter(param);
    }
}
