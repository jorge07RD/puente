//! Colores, medidas y tipografía de Puente.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};

pub const GROUND: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0b);
pub const CARD: Color32 = Color32::from_rgb(0x14, 0x14, 0x15);
pub const RAISED: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x21);
pub const EDGE: Color32 = Color32::from_rgb(0x28, 0x28, 0x2a);
pub const INK: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xef);
pub const INK_SOFT: Color32 = Color32::from_rgb(0xa6, 0xa6, 0xa1);
pub const DOT_OFF: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x4c);
/// El único color: lo activo, lo que está en vivo y lo que pide atención.
pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0x4a, 0x1f);

/// Solo en la animación de carga.
pub const LOADING: Color32 = Color32::from_rgb(0x4a, 0xde, 0x80);

pub const GAP: f32 = 10.0;
pub const PADDING: f32 = 16.0;
pub const CARD_RADIUS: u8 = 14;
pub const BORDER: f32 = 1.0;

pub const TEXT_LABEL: f32 = 12.0;
pub const TEXT_BODY: f32 = 13.0;
pub const TEXT_VALUE: f32 = 20.0;

/// Separación entre letras de las etiquetas en mayúsculas.
pub const LABEL_TRACKING: f32 = 0.8;

/// Familia de la etiqueta y de los botones.
pub fn medium() -> FontFamily {
    FontFamily::Name("martian-medium".into())
}

pub fn install(context: &egui::Context) {
    context.set_fonts(fonts());
    context.global_style_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(TEXT_LABEL, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(TEXT_BODY, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(TEXT_BODY, medium())),
            (TextStyle::Heading, FontId::new(TEXT_VALUE, medium())),
            (TextStyle::Monospace, FontId::new(TEXT_BODY, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(GAP, 6.0);
        style.visuals = visuals();
    });
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "martian".to_string(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/MartianMono-StdRg.ttf"
        ))),
    );
    fonts.font_data.insert(
        "martian-medium".to_string(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/MartianMono-StdMd.ttf"
        ))),
    );

    // Las fuentes de egui quedan detrás: cubren caracteres de las notificaciones
    // que Martian Mono no tiene.
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        list.insert(0, "martian".to_string());
    }
    let mut medium_list = vec!["martian-medium".to_string()];
    medium_list.extend(fallbacks);
    fonts.families.insert(medium(), medium_list);
    fonts
}

fn visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(INK);
    visuals.panel_fill = GROUND;
    visuals.window_fill = CARD;
    visuals.window_stroke = Stroke::new(BORDER, EDGE);
    visuals.extreme_bg_color = GROUND;
    visuals.faint_bg_color = CARD;
    visuals.hyperlink_color = INK;
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.45);
    visuals.selection.stroke = Stroke::new(BORDER, INK);
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let radius = CornerRadius::same(8);
    let widgets = &mut visuals.widgets;
    for state in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    widgets.noninteractive.bg_stroke = Stroke::new(BORDER, EDGE);
    widgets.noninteractive.fg_stroke = Stroke::new(BORDER, INK_SOFT);
    widgets.inactive.weak_bg_fill = CARD;
    widgets.inactive.bg_fill = RAISED;
    widgets.inactive.bg_stroke = Stroke::new(BORDER, EDGE);
    widgets.inactive.fg_stroke = Stroke::new(BORDER, INK);
    widgets.hovered.weak_bg_fill = RAISED;
    widgets.hovered.bg_fill = RAISED;
    widgets.hovered.bg_stroke = Stroke::new(BORDER, INK_SOFT);
    widgets.hovered.fg_stroke = Stroke::new(BORDER, INK);
    widgets.active.weak_bg_fill = EDGE;
    widgets.active.bg_fill = EDGE;
    widgets.active.bg_stroke = Stroke::new(BORDER, INK);
    widgets.active.fg_stroke = Stroke::new(BORDER, INK);
    visuals
}
