//! The look of the window: its colors, sizes and spacing, and the cards its parts sit in.

use nih_plug_egui::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2,
};

/// The background of the window.
pub const BACKGROUND: Color32 = Color32::from_rgb(21, 23, 27);
/// The background of a card.
pub const CARD: Color32 = Color32::from_rgb(30, 33, 39);
/// The edge of a card.
pub const CARD_EDGE: Color32 = Color32::from_rgb(44, 48, 57);
/// The background of fields, lists and the curve.
pub const DEEP: Color32 = Color32::from_rgb(16, 18, 21);
/// The color of what is chosen or active: the curve, the selection, links.
pub const ACCENT: Color32 = Color32::from_rgb(72, 196, 178);
/// The accent, dimmed, for the area under the curve and the selection behind text.
pub const ACCENT_DIM: Color32 = Color32::from_rgba_premultiplied(18, 49, 45, 64);
/// Text.
pub const TEXT: Color32 = Color32::from_rgb(220, 224, 230);
/// Text of less weight: notes, units, labels of the axes.
pub const WEAK: Color32 = Color32::from_rgb(138, 145, 157);
/// Warnings and what the limiter does.
pub const WARM: Color32 = Color32::from_rgb(255, 176, 72);
/// Errors and levels at the top.
pub const HOT: Color32 = Color32::from_rgb(240, 98, 88);
/// Levels with room above them.
pub const GOOD: Color32 = Color32::from_rgb(104, 204, 128);
/// Lines of the grid of the curve.
pub const GRID: Color32 = Color32::from_rgb(42, 46, 54);

/// How round corners are, in logical pixels.
const RADIUS: u8 = 6;

/// Gives the window its look; called once, when it opens.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(10.5, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(11.5, FontFamily::Monospace),
        ),
    ]
    .into();
    let spacing = &mut style.spacing;
    spacing.item_spacing = Vec2::new(8.0, 6.0);
    spacing.button_padding = Vec2::new(10.0, 4.0);
    spacing.interact_size.y = 22.0;
    spacing.window_margin = Margin::same(14);

    let visuals = &mut style.visuals;
    visuals.dark_mode = true;
    visuals.override_text_color = None;
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = CARD;
    visuals.window_stroke = Stroke::new(1.0_f32, CARD_EDGE);
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.extreme_bg_color = DEEP;
    visuals.faint_bg_color = CARD;
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = WARM;
    visuals.error_fg_color = HOT;
    visuals.selection.bg_fill = Color32::from_rgb(28, 86, 78);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.collapsing_header_frame = false;
    let widgets = &mut visuals.widgets;
    let radius = CornerRadius::same(RADIUS);
    widgets.noninteractive.bg_fill = CARD;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, CARD_EDGE);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.noninteractive.corner_radius = radius;
    for (state, fill, edge) in [
        (&mut widgets.inactive, 40, 52),
        (&mut widgets.hovered, 50, 72),
        (&mut widgets.active, 60, 90),
        (&mut widgets.open, 50, 72),
    ] {
        state.bg_fill = Color32::from_rgb(fill, fill + 4, fill + 10);
        state.weak_bg_fill = state.bg_fill;
        state.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(edge, edge + 4, edge + 12));
        state.corner_radius = radius;
    }
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.hovered.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    widgets.active.fg_stroke = Stroke::new(2.0_f32, Color32::WHITE);
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    ctx.set_style(style);
}

/// A card: a part of the window on its own background, with a title.
pub fn card<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_of_height(ui, title, 0.0, add)
}

/// A card at least so high, e.g. so cards side by side end together.
pub fn card_of_height<R>(
    ui: &mut egui::Ui,
    title: &str,
    min_height: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, CARD_EDGE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(min_height);
            if !title.is_empty() {
                ui.label(title_text(title));
            }
            add(ui)
        })
        .inner
}

/// The text of a title of a card or a group: small capitals of a sort, in the weak color.
#[must_use]
pub fn title_text(title: &str) -> egui::RichText {
    egui::RichText::new(title.to_uppercase())
        .size(10.5)
        .strong()
        .color(WEAK)
        .extra_letter_spacing(0.8)
}

/// A note: small and weak.
#[must_use]
pub fn note(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).small().color(WEAK)
}
