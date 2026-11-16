//! The launcher's look: a dark theme in OpenMCM2's orange, and the few
//! widgets it is built from (cards, segmented buttons, switches, the
//! sidebar's pages). Icons are Google's Material Symbols
//! (`egui_material_icons`), drawn inline with the text.

use eframe::egui::{self, Color32, CornerRadius, RichText, Sense, Stroke, Ui, Vec2};
pub use egui_material_icons::icons as icon;
use egui_material_icons::MaterialIcon;

pub const ACCENT: Color32 = Color32::from_rgb(255, 138, 42);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(255, 164, 82);
/// The window behind everything, the sidebar, a card, and what sits
/// sunk into a card (a segmented row's tray): deep blues, lighter the
/// nearer the front.
pub const BG: Color32 = Color32::from_rgb(22, 27, 38);
pub const SIDEBAR: Color32 = Color32::from_rgb(29, 36, 51);
pub const CARD: Color32 = Color32::from_rgb(35, 43, 59);
pub const SUNKEN: Color32 = Color32::from_rgb(26, 33, 48);
pub const LINE: Color32 = Color32::from_rgb(54, 66, 90);
pub const TEXT: Color32 = Color32::from_rgb(245, 247, 251);
pub const WEAK: Color32 = Color32::from_rgb(186, 195, 213);
pub const GOOD: Color32 = Color32::from_rgb(102, 217, 139);
pub const BAD: Color32 = Color32::from_rgb(255, 112, 102);
/// A plain button's face, at rest and hovered.
const BUTTON: Color32 = Color32::from_rgb(48, 59, 82);
const BUTTON_HOVER: Color32 = Color32::from_rgb(62, 75, 104);
/// The cards' icon badges, one each in turn.
const TINTS: [Color32; 6] = [
    ACCENT,
    Color32::from_rgb(79, 195, 247),
    Color32::from_rgb(102, 217, 139),
    Color32::from_rgb(255, 213, 79),
    Color32::from_rgb(255, 111, 174),
    Color32::from_rgb(167, 139, 250),
];

/// Sets the theme and the icons' font on `ctx`.
pub fn apply(ctx: &egui::Context) {
    fonts(ctx);
    // Dark whatever the system's own theme: the colours below are for it.
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.global_style()).clone();
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = SUNKEN;
    v.faint_bg_color = SUNKEN;
    v.code_bg_color = SUNKEN;
    v.override_text_color = Some(TEXT);
    v.weak_text_color = Some(WEAK);
    v.hyperlink_color = ACCENT_HOVER;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.window_corner_radius = CornerRadius::same(12);
    v.window_stroke = Stroke::new(1.0, LINE);
    let round = CornerRadius::same(8);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = round;
        w.fg_stroke = Stroke::new(1.5, TEXT);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.bg_fill = CARD;
    // Buttons' faces (weak_bg_fill), and the sliders' rails and knobs
    // (bg_fill).
    v.widgets.inactive.weak_bg_fill = BUTTON;
    v.widgets.inactive.bg_fill = Color32::from_rgb(70, 84, 114);
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = BUTTON_HOVER;
    v.widgets.hovered.bg_fill = ACCENT_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(96, 112, 146));
    v.widgets.active.weak_bg_fill = Color32::from_rgb(74, 89, 122);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.open.weak_bg_fill = BUTTON_HOVER;
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.slider_width = 320.0;
    style.spacing.slider_rail_height = 6.0;
    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(24.0)),
        (TextStyle::Body, FontId::proportional(15.0)),
        (TextStyle::Button, FontId::proportional(15.0)),
        (TextStyle::Small, FontId::proportional(12.5)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
    // The same for both themes, so the system's light one changes nothing.
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

/// The name of the bold face's font family ([`bold`]).
const BOLD: &str = "inter-bold";

/// Inter (the OFL-licensed font from `shadcn-egui-font-inter`), a
/// variable font: at a regular weight for the text, and at a semi-bold
/// one as its own family for titles and names; egui's own font behind
/// both for anything they lack.
fn fonts(ctx: &egui::Context) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily, VariationCoords};
    let face = |name: &str, weight: u16, families: Vec<InsertFontFamily>| {
        let f = shadcn_egui_font_inter::FONT.face(weight);
        let mut data = egui::FontData::from_static(f.data);
        if let Some(w) = f.wght {
            data.tweak.coords = VariationCoords::new([("wght", w)]);
        }
        FontInsert::new(name, data, families)
    };
    ctx.add_font(face("inter", 420, vec![InsertFontFamily { family: egui::FontFamily::Proportional, priority: FontPriority::Highest }]));
    ctx.add_font(face(BOLD, 620, vec![InsertFontFamily { family: egui::FontFamily::Name(BOLD.into()), priority: FontPriority::Highest }]));
    // The icons behind Inter (whose own glyphs there are letters too);
    // those Inter has glyphs for in the icons' place are not used
    // (`launcher_icons_clear_inter`).
    let mut icons = egui_material_icons::font_insert();
    icons.families = [egui::FontFamily::Proportional, egui::FontFamily::Name(BOLD.into())]
        .into_iter()
        .map(|family| InsertFontFamily { family, priority: FontPriority::Lowest })
        .chain(std::iter::once(InsertFontFamily { family: egui::FontFamily::Name(egui_material_icons::FONT_FAMILY.into()), priority: FontPriority::Highest }))
        .collect();
    ctx.add_font(icons);
}

/// The bold face's family, for `RichText::family`.
pub fn bold_family() -> egui::FontFamily {
    egui::FontFamily::Name(BOLD.into())
}

/// An icon then `text`, as one label.
pub fn with(i: MaterialIcon, text: &str) -> String {
    format!("{}  {text}", String::from(i))
}

/// A card: a rounded panel with an icon and a title, then `body`.
pub fn card<R>(ui: &mut Ui, i: MaterialIcon, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    let r = egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(12)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // The icon in a round badge of its own colour.
                let tint = TINTS[title.bytes().map(|b| b as usize).sum::<usize>() % TINTS.len()];
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 15.0, tint.gamma_multiply(0.22));
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, String::from(i), egui::FontId::proportional(18.0), tint);
                ui.label(RichText::new(title).size(18.0).color(TEXT).family(bold_family()));
            });
            ui.add_space(4.0);
            body(ui)
        })
        .inner;
    ui.add_space(12.0);
    r
}

/// A page's title and what it is for.
pub fn page_title(ui: &mut Ui, title: &str, about: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(28.0).color(TEXT).family(bold_family()));
    ui.add(egui::Label::new(RichText::new(about).color(WEAK).size(15.0)).wrap());
    ui.add_space(10.0);
}

/// A setting's name, and a line on what it does under it; the name's
/// id, to name the control after it (`labelled_by`).
pub fn label(ui: &mut Ui, name: &str, about: &str) -> egui::Id {
    ui.add_space(4.0);
    let mut named = ui.label(RichText::new(name).color(TEXT).size(15.5).family(bold_family()));
    if !about.is_empty() {
        named = named.on_hover_text(about);
    }
    let id = named.id;
    if !about.is_empty() {
        ui.add(egui::Label::new(RichText::new(about).color(WEAK).size(13.5)).wrap());
    }
    id
}

/// Text a little weaker than the rest.
pub fn weak(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(WEAK).size(13.5)
}

/// One of a segmented row's buttons: what it picks, what it says, its icon
/// (if any) and what hovering tells.
pub struct Choice<T> {
    pub value: T,
    pub label: String,
    pub icon: Option<MaterialIcon>,
    pub hover: String,
}

impl<T> Choice<T> {
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Choice { value, label: label.into(), icon: None, hover: String::new() }
    }
    pub fn icon(mut self, i: MaterialIcon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn hover(mut self, text: impl Into<String>) -> Self {
        self.hover = text.into();
        self
    }
}

/// A row of buttons, one of them picked (filled in the accent), in a
/// sunken tray; wrapping where narrow. The one clicked, if any.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: T, choices: &[Choice<T>]) -> Option<T> {
    let mut picked = None;
    egui::Frame::new().fill(SUNKEN).corner_radius(10).inner_margin(4).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for c in choices {
                let on = c.value == current;
                let text = match c.icon {
                    Some(i) => with(i, &c.label),
                    None => c.label.clone(),
                };
                let colour = if on { Color32::WHITE } else if ui.is_enabled() { TEXT } else { WEAK };
                let b = egui::Button::new(RichText::new(text).color(colour))
                    .fill(if on { ACCENT } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .corner_radius(7)
                    .min_size(egui::vec2(0.0, 32.0));
                let mut r = ui.add(b);
                if !c.hover.is_empty() {
                    r = r.on_hover_text(&c.hover);
                }
                if r.clicked() {
                    picked = Some(c.value);
                }
            }
        });
    });
    picked
}

/// An on/off switch.
pub fn switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let size = Vec2::new(42.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, *on);
        let enabled = ui.is_enabled();
        let off = if response.hovered() { Color32::from_rgb(92, 108, 140) } else { Color32::from_rgb(74, 88, 118) };
        let mut track = lerp(off, ACCENT, t);
        let mut knob = Color32::WHITE;
        if !enabled {
            track = track.gamma_multiply(0.4);
            knob = knob.gamma_multiply(0.5);
        }
        let r = rect.height() / 2.0;
        ui.painter().rect_filled(rect, r, track);
        let x = egui::lerp((rect.left() + r)..=(rect.right() - r), t);
        ui.painter().circle_filled(egui::pos2(x, rect.center().y), r - 3.0, knob);
    }
    response
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// A switch with its name and a line on what it does beside it, in a
/// tile; whether it was flipped.
pub fn switch_tile(ui: &mut Ui, on: &mut bool, name: &str, about: &str, width: f32) -> egui::Response {
    // The whole tile says what it does when the pointer is on it.
    let tile = egui::Frame::new()
        .fill(SUNKEN)
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(width - 24.0);
            // All one height, so a row of them lines up.
            ui.set_min_height(46.0);
            ui.horizontal_top(|ui| {
                let r = switch(ui, on);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(name).color(TEXT).family(bold_family()));
                    ui.add(egui::Label::new(weak(about)).wrap());
                });
                r
            })
            .inner
        });
    if !about.is_empty() {
        tile.response.on_hover_text(about);
    }
    tile.inner.on_hover_text(about)
}

/// A page in the sidebar: its icon and name, filled when it shows.
pub fn nav(ui: &mut Ui, on: bool, i: MaterialIcon, name: &str) -> bool {
    let text = RichText::new(with(i, name)).size(15.5).color(if on { Color32::WHITE } else { TEXT });
    let b = egui::Button::new(text)
        .fill(if on { Color32::from_rgb(52, 64, 90) } else { Color32::TRANSPARENT })
        .stroke(Stroke::NONE)
        .corner_radius(8)
        .min_size(egui::vec2(ui.available_width(), 38.0));
    let r = ui.add(b);
    if on {
        // An accent bar at its left edge.
        let rect = r.rect;
        ui.painter().rect_filled(egui::Rect::from_min_size(rect.left_top() + egui::vec2(0.0, 8.0), egui::vec2(3.0, rect.height() - 16.0)), 2.0, ACCENT);
    }
    r.clicked()
}

/// A big accent button: the label, how high, whether it can be pressed.
pub fn big_button(ui: &mut Ui, text: &str, height: f32, enabled: bool) -> egui::Response {
    let b = egui::Button::new(RichText::new(text).size(20.0).family(bold_family()).color(Color32::WHITE))
        .fill(if enabled { ACCENT } else { Color32::from_rgb(64, 76, 100) })
        .stroke(Stroke::NONE)
        .corner_radius(10)
        .min_size(egui::vec2(ui.available_width(), height));
    ui.add_enabled(enabled, b)
}

/// A small coloured status line: an icon and the text, good or bad.
pub fn status(ui: &mut Ui, good: bool, text: &str) {
    let (i, c) = if good { (icon::ICON_CHECK_CIRCLE, GOOD) } else { (icon::ICON_ERROR, BAD) };
    ui.add(egui::Label::new(RichText::new(with(i, text)).color(c)).wrap());
}

/// A small rounded tag of `text`, for a summary.
pub fn chip(ui: &mut Ui, text: &str) {
    let b = egui::Button::new(RichText::new(text).size(13.5))
        .fill(SUNKEN)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(14)
        .sense(Sense::hover());
    ui.add(b);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inter has glyphs of its own at some of the icons' code points, and
    /// as it comes first those would show in place of the icon: none of
    /// the icons the launcher uses may be one.
    #[test]
    fn launcher_icons_clear_inter() {
        let ctx = egui::Context::default();
        let mut defs = egui::FontDefinitions::empty();
        defs.font_data.insert("inter".into(), std::sync::Arc::new(egui::FontData::from_static(shadcn_egui_font_inter::FONT.face(400).data)));
        defs.families.insert(egui::FontFamily::Proportional, vec!["inter".into()]);
        ctx.set_fonts(defs);
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        let used = [("ICON_DISC_FULL", icon::ICON_DISC_FULL), ("ICON_LAN", icon::ICON_LAN), ("ICON_EXTENSION", icon::ICON_EXTENSION), ("ICON_MEMORY", icon::ICON_MEMORY), ("ICON_SPORTS_ESPORTS", icon::ICON_SPORTS_ESPORTS), ("ICON_AUTO_AWESOME", icon::ICON_AUTO_AWESOME), ("ICON_AUTO_FIX_HIGH", icon::ICON_AUTO_FIX_HIGH), ("ICON_CHECK_CIRCLE", icon::ICON_CHECK_CIRCLE), ("ICON_CLOUDY", icon::ICON_CLOUDY), ("ICON_DELETE", icon::ICON_DELETE), ("ICON_DOWNLOAD", icon::ICON_DOWNLOAD), ("ICON_DOWNLOADING", icon::ICON_DOWNLOADING), ("ICON_ERROR", icon::ICON_ERROR), ("ICON_FLARE", icon::ICON_FLARE), ("ICON_DRIVE_FOLDER_UPLOAD", icon::ICON_DRIVE_FOLDER_UPLOAD), ("ICON_IMAGE", icon::ICON_IMAGE), ("ICON_HISTORY", icon::ICON_HISTORY), ("ICON_INFO", icon::ICON_INFO), ("ICON_LANDSCAPE", icon::ICON_LANDSCAPE), ("ICON_LIGHT_MODE", icon::ICON_LIGHT_MODE), ("ICON_MONITOR", icon::ICON_MONITOR), ("ICON_NIGHTS_STAY", icon::ICON_NIGHTS_STAY), ("ICON_PALETTE", icon::ICON_PALETTE), ("ICON_PLAY_CIRCLE", icon::ICON_PLAY_CIRCLE), ("ICON_RAINY", icon::ICON_RAINY), ("ICON_REFRESH", icon::ICON_REFRESH), ("ICON_ROUTE", icon::ICON_ROUTE), ("ICON_SCHEDULE", icon::ICON_SCHEDULE), ("ICON_SEARCH", icon::ICON_SEARCH), ("ICON_SHORTCUT", icon::ICON_SHORTCUT), ("ICON_SPEED", icon::ICON_SPEED), ("ICON_SPORTS_MOTORSPORTS", icon::ICON_SPORTS_MOTORSPORTS), ("ICON_SUNNY", icon::ICON_SUNNY), ("ICON_TEXTURE", icon::ICON_TEXTURE), ("ICON_TUNE", icon::ICON_TUNE), ("ICON_UPDATE", icon::ICON_UPDATE), ("ICON_WB_SUNNY", icon::ICON_WB_SUNNY), ("ICON_WB_TWILIGHT", icon::ICON_WB_TWILIGHT), ];
        let clash: Vec<&str> = used
            .iter()
            .filter(|(_, i)| ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(14.0), String::from(*i).chars().next().unwrap())))
            .map(|(n, _)| *n)
            .collect();
        assert!(clash.is_empty(), "Inter draws these in the icons' place: {clash:?}");
    }
}
