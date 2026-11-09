//! El dibujo de puntos: cifras en matriz, anillo, onda, puente y silueta del móvil.

use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::{Color32, Painter, Pos2, Rect, Vec2, pos2, vec2};

use crate::theme::{ACCENT, DOT_OFF, INK};

const GLYPH_HEIGHT: usize = 7;
/// Por debajo de esta distancia entre puntos las cifras se empastan.
pub const MIN_TEXT_PITCH: f32 = 5.0;
/// Tamaño de cada punto respecto a la distancia entre puntos.
const DOT_RADIUS_RATIO: f32 = 0.34;

/// Filas de cada carácter, de arriba abajo; cada bit es una columna de 5.
const fn glyph(character: char) -> Option<[u8; GLYPH_HEIGHT]> {
    Some(match character {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '%' => [0b11001, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b10011],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        _ => return None,
    })
}

/// Ancho en columnas: las cifras son todas iguales para que no bailen al cambiar.
fn columns(character: char) -> usize {
    match character {
        '.' | ':' => 1,
        ' ' => 3,
        _ => 5,
    }
}

pub fn text_size(text: &str, pitch: f32) -> Vec2 {
    let columns_used: usize = text.chars().map(|character| columns(character) + 1).sum();
    vec2(
        columns_used.saturating_sub(1) as f32 * pitch,
        (GLYPH_HEIGHT - 1) as f32 * pitch,
    )
}

/// Dibuja `text` desde su esquina superior izquierda. Los caracteres sin matriz
/// se dejan en blanco, así que solo sirve para cifras y palabras cortas.
pub fn paint_text(painter: &Painter, origin: Pos2, text: &str, pitch: f32, color: Color32) {
    debug_assert!(pitch >= MIN_TEXT_PITCH, "cifras ilegibles a {pitch}");
    let radius = pitch * DOT_RADIUS_RATIO;
    let mut x = origin.x;
    for character in text.chars() {
        match character {
            '.' => {
                painter.circle_filled(
                    pos2(x, origin.y + (GLYPH_HEIGHT - 1) as f32 * pitch),
                    radius,
                    color,
                );
            }
            ':' => {
                for row in [2, 4] {
                    painter.circle_filled(pos2(x, origin.y + row as f32 * pitch), radius, color);
                }
            }
            _ => paint_glyph(painter, pos2(x, origin.y), character, pitch, radius, color),
        }
        x += (columns(character) + 1) as f32 * pitch;
    }
}

fn paint_glyph(painter: &Painter, origin: Pos2, character: char, pitch: f32, radius: f32, color: Color32) {
    let Some(rows) = glyph(character) else {
        return;
    };
    for (row, bits) in rows.iter().enumerate() {
        for column in 0..5 {
            if bits & (0b10000 >> column) != 0 {
                let center = origin + vec2(column as f32 * pitch, row as f32 * pitch);
                painter.circle_filled(center, radius, color);
            }
        }
    }
}

/// Anillo que empieza arriba y avanza en sentido horario. El último punto
/// encendido va en el acento cuando `head` es verdadero.
pub fn paint_ring(painter: &Painter, center: Pos2, radius: f32, dots: usize, fraction: f32, head: bool) {
    let lit = (fraction.clamp(0.0, 1.0) * dots as f32).round() as usize;
    let size = TAU * radius / dots as f32 * DOT_RADIUS_RATIO;
    for index in 0..dots {
        let angle = index as f32 / dots as f32 * TAU - FRAC_PI_2;
        let position = center + vec2(angle.cos(), angle.sin()) * radius;
        let is_lit = index < lit;
        let color = match (is_lit, head && index + 1 == lit) {
            (true, true) => ACCENT,
            (true, false) => INK,
            (false, _) => DOT_OFF,
        };
        painter.circle_filled(position, if is_lit { size } else { size * 0.7 }, color);
    }
}

/// Onda espejada: cada columna apila puntos según su nivel, de 0 a 1.
pub fn paint_wave(painter: &Painter, rect: Rect, levels: &[f32], pitch: f32) {
    if levels.is_empty() {
        return;
    }
    let radius = pitch * DOT_RADIUS_RATIO;
    let middle = rect.center().y;
    let peak_rows = ((rect.height() / 2.0 / pitch).floor() as usize).max(1);
    let loudest = levels
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .filter(|(_, level)| **level > 0.05)
        .map(|(index, _)| index);

    let step = rect.width() / levels.len() as f32;
    for (index, level) in levels.iter().enumerate() {
        let x = rect.left() + step * (index as f32 + 0.5);
        let rows = (level.clamp(0.0, 1.0) * peak_rows as f32).round() as usize;
        painter.circle_filled(pos2(x, middle), radius, if rows == 0 { DOT_OFF } else { INK });
        let color = if loudest == Some(index) { ACCENT } else { INK };
        for row in 1..=rows {
            let offset = row as f32 * pitch;
            painter.circle_filled(pos2(x, middle - offset), radius, color);
            painter.circle_filled(pos2(x, middle + offset), radius, color);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BridgeState {
    /// Tendido desde la PC hasta `reach`, de 0 a 1, buscando la otra orilla.
    Reaching { reach: f32 },
    /// Completo y sin tráfico.
    Linked,
    /// Completo, con un paquete en la posición `position`, de 0 a 1.
    Traffic { position: f32 },
}

/// El motivo de la app: un puente de puntos entre la PC y el móvil.
pub fn paint_bridge(painter: &Painter, rect: Rect, state: BridgeState) {
    const DECK_COLUMNS: usize = 41;
    const HANGER_EVERY: usize = 4;

    let pitch = rect.width() / (DECK_COLUMNS - 1) as f32;
    let radius = pitch * DOT_RADIUS_RATIO;
    let deck_y = rect.bottom() - pitch;
    let arch_height = (rect.height() - pitch * 3.0).max(pitch);

    let reach = match state {
        BridgeState::Reaching { reach } => reach.clamp(0.0, 1.0),
        _ => 1.0,
    };
    let last_built = (reach * (DECK_COLUMNS - 1) as f32).round() as usize;

    for column in 0..DECK_COLUMNS {
        let x = rect.left() + column as f32 * pitch;
        let built = column <= last_built;
        let color = if built { INK } else { DOT_OFF };

        painter.circle_filled(pos2(x, deck_y), radius, color);

        let along = column as f32 / (DECK_COLUMNS - 1) as f32;
        let curve = 1.0 - (2.0 * along - 1.0).powi(2);
        let arch_y = deck_y - curve * arch_height;
        if built {
            painter.circle_filled(pos2(x, arch_y), radius, INK);
        }

        if built && column % HANGER_EVERY == 0 {
            let mut y = deck_y - pitch;
            while y > arch_y + pitch * 0.5 {
                painter.circle_filled(pos2(x, y), radius * 0.7, INK.gamma_multiply(0.45));
                y -= pitch;
            }
        }
    }

    match state {
        BridgeState::Reaching { .. } => {
            let tip = rect.left() + last_built as f32 * pitch;
            painter.circle_filled(pos2(tip, deck_y), radius * 1.7, ACCENT);
        }
        BridgeState::Traffic { position } => {
            let x = rect.left() + position.clamp(0.0, 1.0) * rect.width();
            painter.circle_filled(pos2(x, deck_y), radius * 1.7, ACCENT);
        }
        BridgeState::Linked => {}
    }
}

/// Círculos concéntricos de puntos. Con `pulse`, de 0 a 1, una onda los recorre
/// desde el centro hacia afuera.
pub fn paint_radar(painter: &Painter, center: Pos2, inner: f32, outer: f32, rings: usize, pulse: Option<f32>) {
    const RING_SPACING_DOTS: f32 = 7.0;
    for ring in 0..rings {
        let along = ring as f32 / (rings.max(2) - 1) as f32;
        let radius = inner + (outer - inner) * along;
        let dots = ((TAU * radius / RING_SPACING_DOTS).round() as usize).max(6);
        let (color, size) = match pulse {
            Some(pulse) if (along - pulse).abs() < 0.18 => (ACCENT, 1.8),
            _ => (INK.gamma_multiply(1.0 - along * 0.6), 1.3),
        };
        for index in 0..dots {
            let angle = index as f32 / dots as f32 * TAU;
            painter.circle_filled(center + vec2(angle.cos(), angle.sin()) * radius, size, color);
        }
    }
}

/// Barra de puntos para el avance de una tarea, de 0 a 1.
pub fn paint_progress(painter: &Painter, rect: Rect, pitch: f32, fraction: f32) {
    let radius = pitch * DOT_RADIUS_RATIO;
    let dots = ((rect.width() / pitch).floor() as usize).max(2);
    let lit = (fraction.clamp(0.0, 1.0) * dots as f32).round() as usize;
    for index in 0..dots {
        let color = match index + 1 {
            position if position == lit => ACCENT,
            position if position < lit => INK,
            _ => DOT_OFF,
        };
        let position = pos2(rect.left() + index as f32 * pitch, rect.center().y);
        painter.circle_filled(position, radius, color);
    }
}

/// Contorno redondeado de puntos.
pub fn paint_outline(painter: &Painter, rect: Rect, pitch: f32, corner: usize, color: Color32) {
    let radius = pitch * DOT_RADIUS_RATIO;
    let columns = (rect.width() / pitch).floor() as usize;
    let rows = (rect.height() / pitch).floor() as usize;
    if columns < corner * 2 + 1 || rows < corner * 2 + 1 {
        return;
    }
    let origin = rect.left_top()
        + vec2(
            (rect.width() - columns as f32 * pitch) / 2.0,
            (rect.height() - rows as f32 * pitch) / 2.0,
        );

    let is_corner = |column: usize, row: usize| {
        let near_x = column.min(columns - column);
        let near_y = row.min(rows - row);
        near_x + near_y < corner
    };
    for row in 0..=rows {
        for column in 0..=columns {
            let on_edge = row == 0 || row == rows || column == 0 || column == columns;
            let rounded_edge = {
                let near_x = column.min(columns - column);
                let near_y = row.min(rows - row);
                near_x + near_y == corner
            };
            if (on_edge && !is_corner(column, row)) || rounded_edge {
                let position = origin + vec2(column as f32 * pitch, row as f32 * pitch);
                painter.circle_filled(position, radius, color);
            }
        }
    }
}

/// Silueta de un móvil en puntos, para cuando todavía no llega su pantalla.
pub fn paint_phone(painter: &Painter, rect: Rect, pitch: f32) {
    paint_outline(painter, rect, pitch, 3, INK.gamma_multiply(0.75));
    let top_center = pos2(rect.center().x, rect.top() + pitch * 2.0);
    painter.circle_filled(top_center, pitch * DOT_RADIUS_RATIO * 1.6, ACCENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_all_have_the_same_width_so_figures_do_not_jump() {
        for digit in '0'..='9' {
            assert!(glyph(digit).is_some(), "falta la cifra {digit}");
            assert_eq!(columns(digit), 5);
        }
        assert_eq!(text_size("111", 2.0), text_size("888", 2.0));
    }

    #[test]
    fn the_whole_alphabet_has_a_glyph() {
        for character in 'A'..='Z' {
            assert!(glyph(character).is_some(), "falta el carácter {character}");
        }
    }

    #[test]
    fn every_glyph_fits_five_columns() {
        for character in ('0'..='9').chain('A'..='Z').chain(['%', '-']) {
            let rows = glyph(character).expect("el carácter existe");
            assert!(rows.iter().all(|row| *row < 0b100000), "{character}");
        }
    }
}
