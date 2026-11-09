//! Iconos propios, dibujados con un solo trazo.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

const STROKE_RATIO: f32 = 0.09;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Back,
    Home,
    Recents,
    Play,
    Pause,
    Previous,
    Next,
    Close,
    Stop,
    Bell,
    Panels,
    Check,
    Unlink,
}

/// Convierte coordenadas de 0 a 1 en puntos dentro de `rect`.
fn point(rect: Rect, x: f32, y: f32) -> Pos2 {
    pos2(
        rect.left() + rect.width() * x,
        rect.top() + rect.height() * y,
    )
}

pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let stroke = Stroke::new((rect.width() * STROKE_RATIO).max(1.25), color);
    let at = |x, y| point(rect, x, y);
    let filled = |points: Vec<Pos2>| {
        painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
    };
    let line = |points: Vec<Pos2>| {
        painter.add(Shape::line(points, stroke));
    };

    match icon {
        Icon::Back => line(vec![at(0.62, 0.2), at(0.3, 0.5), at(0.62, 0.8)]),
        Icon::Home => {
            painter.circle_stroke(rect.center(), rect.width() * 0.3, stroke);
        }
        Icon::Recents => {
            painter.rect_stroke(
                Rect::from_center_size(rect.center(), rect.size() * 0.56),
                2.0,
                stroke,
                eframe::egui::StrokeKind::Middle,
            );
        }
        Icon::Play => filled(vec![at(0.32, 0.2), at(0.8, 0.5), at(0.32, 0.8)]),
        Icon::Pause => {
            for x in [0.38, 0.62] {
                line(vec![at(x, 0.24), at(x, 0.76)]);
            }
        }
        Icon::Previous => {
            filled(vec![at(0.74, 0.24), at(0.36, 0.5), at(0.74, 0.76)]);
            line(vec![at(0.28, 0.24), at(0.28, 0.76)]);
        }
        Icon::Next => {
            filled(vec![at(0.26, 0.24), at(0.64, 0.5), at(0.26, 0.76)]);
            line(vec![at(0.72, 0.24), at(0.72, 0.76)]);
        }
        Icon::Stop => {
            painter.rect_filled(Rect::from_center_size(rect.center(), rect.size() * 0.5), 2.0, color);
        }
        Icon::Bell => {
            line(vec![
                at(0.24, 0.68),
                at(0.32, 0.56),
                at(0.34, 0.36),
                at(0.5, 0.2),
                at(0.66, 0.36),
                at(0.68, 0.56),
                at(0.76, 0.68),
                at(0.24, 0.68),
            ]);
            line(vec![at(0.43, 0.82), at(0.57, 0.82)]);
        }
        Icon::Panels => {
            painter.rect_stroke(
                Rect::from_center_size(rect.center(), rect.size() * 0.72),
                2.0,
                stroke,
                eframe::egui::StrokeKind::Middle,
            );
            for x in [0.36, 0.64] {
                line(vec![at(x, 0.14), at(x, 0.86)]);
            }
        }
        Icon::Check => line(vec![at(0.2, 0.54), at(0.42, 0.76), at(0.8, 0.28)]),
        Icon::Unlink => {
            for x in [0.24, 0.76] {
                painter.circle_stroke(at(x, 0.5), rect.width() * 0.16, stroke);
            }
            line(vec![at(0.42, 0.84), at(0.58, 0.16)]);
        }
        Icon::Close => {
            line(vec![at(0.28, 0.28), at(0.72, 0.72)]);
            line(vec![at(0.72, 0.28), at(0.28, 0.72)]);
        }
    }
}
