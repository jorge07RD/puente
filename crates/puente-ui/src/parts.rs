//! Las piezas de la interfaz: tarjeta, etiqueta, cifra y botones.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, UiBuilder, vec2,
};

use unicode_normalization::UnicodeNormalization;

use crate::dots;
use crate::icons::{self, Icon};
use crate::theme::{
    ACCENT, BORDER, CARD, CARD_RADIUS, EDGE, INK, INK_SOFT, LABEL_TRACKING, PADDING, RAISED, TEXT_LABEL,
    TEXT_VALUE, medium,
};

const HEADER_GAP: f32 = 10.0;

/// Las letras decorativas Unicode, muy usadas en nombres de redes, se convierten
/// en letras normales.
fn plain_letters(text: &str) -> String {
    text.nfkc().collect()
}

/// Texto ajeno listo para mostrar: sin los caracteres que ninguna fuente puede dibujar.
pub fn displayable(ui: &egui::Ui, text: &str) -> String {
    let font = FontId::new(TEXT_LABEL, egui::FontFamily::Proportional);
    let plain = plain_letters(text);
    let kept: String = ui.ctx().fonts_mut(|fonts| {
        plain
            .chars()
            .filter(|character| character.is_whitespace() || fonts.has_glyph(&font, *character))
            .collect()
    });
    kept.trim().to_string()
}

/// Etiqueta en mayúsculas, como en un instrumento.
pub fn label_text(text: &str) -> RichText {
    RichText::new(text.to_uppercase())
        .font(FontId::new(TEXT_LABEL, medium()))
        .extra_letter_spacing(LABEL_TRACKING)
        .color(INK_SOFT)
}

pub fn label(ui: &mut egui::Ui, text: &str) -> Response {
    ui.label(label_text(text))
}

fn card_header(ui: &mut egui::Ui, index: &str, title: &str, live: bool, trailing: &str) {
    ui.horizontal(|ui| {
        label(ui, &format!("{index} {title}"));
        if live {
            let (dot, _) = ui.allocate_exact_size(vec2(8.0, TEXT_LABEL), Sense::hover());
            ui.painter().circle_filled(dot.center(), 3.5, ACCENT);
        }
        if !trailing.is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                label(ui, trailing);
            });
        }
    });
    ui.add_space(HEADER_GAP);
}

/// Tarjeta en un lugar exacto, para que los bordes de las columnas coincidan.
pub fn card_full(
    ui: &mut egui::Ui,
    rect: Rect,
    index: &str,
    title: &str,
    live: bool,
    trailing: &str,
    body: impl FnOnce(&mut egui::Ui),
) {
    ui.painter().rect(
        rect,
        CornerRadius::same(CARD_RADIUS),
        CARD,
        Stroke::new(BORDER, EDGE),
        StrokeKind::Inside,
    );

    let inner = rect.shrink(PADDING);
    let mut content = ui.new_child(UiBuilder::new().max_rect(inner));
    content.set_clip_rect(rect.intersect(ui.clip_rect()));
    card_header(&mut content, index, title, live, trailing);
    body(&mut content);
}

pub fn card_at(
    ui: &mut egui::Ui,
    rect: Rect,
    index: &str,
    title: &str,
    live: bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    card_full(ui, rect, index, title, live, "", body);
}

/// Tarjeta que crece con su contenido, para la columna del móvil.
pub fn card(ui: &mut egui::Ui, index: &str, title: &str, live: bool, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(BORDER, EDGE))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(PADDING)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            card_header(ui, index, title, live, "");
            body(ui);
        });
}

/// Cifra grande en matriz de puntos, con su unidad al lado.
pub fn dot_figure(ui: &mut egui::Ui, value: &str, unit: &str, pitch: f32, color: Color32) -> Response {
    let size = dots::text_size(value, pitch);
    let unit_width = if unit.is_empty() { 0.0 } else { 8.0 + unit.chars().count() as f32 * 9.0 };
    let radius_margin = pitch * 0.5;
    let total = vec2(size.x + unit_width + radius_margin * 2.0, size.y + radius_margin * 2.0);
    let (rect, response) = ui.allocate_exact_size(total, Sense::hover());
    let origin = rect.left_top() + vec2(radius_margin, radius_margin);
    dots::paint_text(ui.painter(), origin, value, pitch, color);
    if !unit.is_empty() {
        ui.painter().text(
            rect.right_bottom() - vec2(0.0, radius_margin),
            Align2::RIGHT_BOTTOM,
            unit,
            FontId::new(TEXT_LABEL, medium()),
            INK_SOFT,
        );
    }
    response
}

/// Etiqueta sobre valor, para las medidas secundarias.
pub fn stat(ui: &mut egui::Ui, title: &str, value: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        label(ui, title);
        ui.label(RichText::new(value).size(TEXT_VALUE).color(INK));
    });
}

fn paint_focus(ui: &egui::Ui, rect: Rect, radius: f32, response: &Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            radius + 2.0,
            Stroke::new(1.5, INK),
            StrokeKind::Outside,
        );
    }
}

/// Botón con forma de píldora. `primary` lo marca con el acento.
pub fn pill(ui: &mut egui::Ui, text: &str, height: f32, primary: bool) -> Response {
    const SIDE: f32 = 14.0;

    let foreground = if primary { ACCENT } else { INK };
    let galley = ui.painter().layout_no_wrap(
        text.to_uppercase(),
        FontId::new(TEXT_LABEL, medium()),
        foreground,
    );
    let size = vec2(galley.size().x + SIDE * 2.0, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let radius = height / 2.0;

    let (fill, edge) = if response.is_pointer_button_down_on() {
        (EDGE, if primary { ACCENT } else { INK })
    } else if response.hovered() {
        (RAISED, if primary { ACCENT } else { INK_SOFT })
    } else {
        (CARD, if primary { ACCENT } else { EDGE })
    };
    ui.painter().rect(rect, radius, fill, Stroke::new(BORDER, edge), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, foreground);
    paint_focus(ui, rect, radius, &response);
    response
}

pub fn pill_button(ui: &mut egui::Ui, text: &str) -> Response {
    pill(ui, text, 32.0, false)
}

/// Fila con un interruptor a la derecha.
pub fn switch_row(ui: &mut egui::Ui, text: &str, on: bool, height: f32) -> Response {
    const TRACK: egui::Vec2 = vec2(34.0, 18.0);

    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    ui.painter().text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        text.to_uppercase(),
        FontId::new(TEXT_LABEL, medium()),
        INK_SOFT,
    );

    let track = Rect::from_center_size(rect.right_center() - vec2(TRACK.x / 2.0, 0.0), TRACK);
    let edge = if response.hovered() { INK_SOFT } else { EDGE };
    ui.painter().rect_stroke(track, TRACK.y / 2.0, Stroke::new(BORDER, edge), StrokeKind::Inside);
    let knob_x = if on { track.right() - TRACK.y / 2.0 } else { track.left() + TRACK.y / 2.0 };
    ui.painter().circle_filled(
        egui::pos2(knob_x, track.center().y),
        5.5,
        if on { ACCENT } else { INK_SOFT },
    );
    paint_focus(ui, rect, 6.0, &response);
    response
}

/// Botón redondo con icono. `primary` lo marca con el acento.
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, diameter: f32, primary: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(diameter), Sense::click());
    paint_icon_button(ui, rect, icon, primary, response)
}

/// Igual que [`icon_button`], en un lugar exacto.
pub fn icon_button_at(ui: &mut egui::Ui, rect: Rect, icon: Icon, primary: bool) -> Response {
    let response = ui.allocate_rect(rect, Sense::click());
    paint_icon_button(ui, rect, icon, primary, response)
}

fn paint_icon_button(ui: &egui::Ui, rect: Rect, icon: Icon, primary: bool, response: Response) -> Response {
    let diameter = rect.width();
    let center = rect.center();
    let radius = diameter / 2.0;
    let color = if primary { ACCENT } else { INK };

    if response.is_pointer_button_down_on() {
        ui.painter().circle_filled(center, radius, EDGE);
    } else if response.hovered() {
        ui.painter().circle_filled(center, radius, RAISED);
    }
    if primary {
        ui.painter().circle_stroke(center, radius - 1.0, Stroke::new(1.5, ACCENT));
    }

    let glyph = Rect::from_center_size(center, egui::Vec2::splat(diameter * 0.46));
    icons::paint(ui.painter(), glyph, icon, color);
    paint_focus(ui, rect, radius, &response);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorative_letters_become_plain_and_spanish_is_untouched() {
        assert_eq!(plain_letters("\u{1D407}\u{1D428}\u{1D425}\u{1D41A}"), "Hola");
        assert_eq!(plain_letters("Cañón, ¿qué tal?"), "Cañón, ¿qué tal?");
    }
}
