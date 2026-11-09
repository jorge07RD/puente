//! La ventana de la PC: el móvil en el centro y sus instrumentos alrededor.

use std::path::PathBuf;

use eframe::egui::{
    self, Align, CursorIcon, Event, Key, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    TextureHandle, UiBuilder, Vec2, pos2, vec2,
};
use puente_core::wire::{Input, KeyPress, MediaAction, NavAction, NotificationInfo, PhoneCommand, TouchPhase};
use puente_core::{Engine, Status, View};

use crate::{DesktopActions, RecentFile};
use crate::dots::{self, BridgeState};
use crate::motion::{self, LinkMotion};
use crate::icons::{self, Icon};
use crate::parts::{
    self, card_full, displayable, dot_figure, icon_button, icon_button_at, label, pill, pill_button, stat, switch_row,
};
use crate::theme::{ACCENT, EDGE, GAP, INK, INK_SOFT, LOADING, PADDING, TEXT_LABEL};

const OUTER: f32 = 10.0;
const SIDE_WIDTH: f32 = 248.0;
const SIDE_MAX_WIDTH: f32 = 340.0;
/// Separación entre el móvil y las tarjetas cuando sobra sitio.
const CENTER_GUTTER: f32 = 24.0;
const CARD_COUNT: usize = 8;
/// A partir de esta proporción de ventana, las tarjetas van en dos filas.
const WIDE_RATIO: f32 = 1.1;
const GRID_MIN_CARD_HEIGHT: f32 = 150.0;
const CARD_MOVE_SECONDS: f32 = 0.35;
const RECENT_ROW_HEIGHT: f32 = 18.0;
const TOOLBAR_HEIGHT: f32 = 56.0;
const LANDSCAPE_ROW_HEIGHT: f32 = 168.0;
const MUSIC_HEIGHT: f32 = 156.0;
const FILES_HEIGHT: f32 = 204.0;
const SETTINGS_HEIGHT: f32 = 128.0;
/// Lo mismo que tarda el móvil en callarse solo.
const RING_SECONDS: f32 = 30.0;
const STACKED_CARD_HEIGHT: f32 = 196.0;
/// Por debajo de este ancho no caben las columnas junto al móvil.
const COLUMNS_MIN_WIDTH: f32 = 860.0;
const COLUMNS_MIN_HEIGHT: f32 = 600.0;
const WAVE_COLUMNS: usize = 34;
const SCREEN_RADIUS: u8 = 16;
/// Por debajo de este nivel se considera silencio.
const SILENCE_LEVEL: f32 = 0.02;
const NAVIGATION_WIDTH: f32 = 3.0 * 44.0 + 2.0 * 28.0;
const TRANSFER_REFRESH: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Default)]
pub struct DesktopState {
    touching: bool,
    /// Cuándo se calla el móvil, si está sonando.
    ring_ends: Option<f32>,
    /// Ventana pequeña: se están mostrando las tarjetas en lugar del móvil.
    show_cards: bool,
    /// Solo la transmisión, sin tarjetas.
    focus: bool,
    /// Hasta cuándo vale el segundo toque para desvincular.
    unpair_until: Option<f32>,
    link: LinkMotion,
    cards: CardMotion,
}

impl DesktopState {
    pub fn is_settling(&self, time: f32) -> bool {
        self.link.settling(time) || self.cards.moving()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Arrangement {
    /// Móvil en vertical: columnas a los lados.
    Columns {
        screen: Rect,
        left: [Rect; 4],
        right: [Rect; 4],
    },
    /// Móvil en horizontal: columna a la derecha y fila debajo.
    Landscape {
        screen: Rect,
        bottom: [Rect; 4],
        right: [Rect; 4],
    },
    /// Sin transmisión: solo las tarjetas, ocupando toda la ventana.
    Grid { cards: [Rect; CARD_COUNT] },
    /// Ventana pequeña: el móvil o las tarjetas, una cosa cada vez.
    Single { area: Rect },
}

impl Arrangement {
    /// Dónde va cada tarjeta, si esta disposición las muestra a todas.
    fn card_rects(&self) -> Option<[Rect; CARD_COUNT]> {
        let joined = |first: &[Rect; 4], second: &[Rect; 4]| {
            std::array::from_fn(|index| if index < 4 { first[index] } else { second[index - 4] })
        };
        match self {
            Self::Columns { left, right, .. } => Some(joined(left, right)),
            Self::Landscape { bottom, right, .. } => Some(joined(bottom, right)),
            Self::Grid { cards } => Some(*cards),
            Self::Single { .. } => None,
        }
    }
}

fn grid(area: Rect) -> Option<[Rect; CARD_COUNT]> {
    let (columns, rows) = if area.width() / area.height() >= WIDE_RATIO { (4, 2) } else { (2, 4) };
    let width = (area.width() - GAP * (columns - 1) as f32) / columns as f32;
    let height = (area.height() - GAP * (rows - 1) as f32) / rows as f32;
    if width < SIDE_WIDTH || height < GRID_MIN_CARD_HEIGHT {
        return None;
    }
    Some(std::array::from_fn(|index| {
        let origin = area.min
            + vec2(
                (index % columns) as f32 * (width + GAP),
                (index / columns) as f32 * (height + GAP),
            );
        Rect::from_min_size(origin, vec2(width, height))
    }))
}

/// Lleva las tarjetas de una disposición a otra cuando empieza o termina la transmisión.
#[derive(Default)]
struct CardMotion {
    previous: Option<[Rect; CARD_COUNT]>,
    from: Option<[Rect; CARD_COUNT]>,
    streaming: bool,
    started: f32,
}

impl CardMotion {
    fn place(&mut self, targets: [Rect; CARD_COUNT], streaming: bool, time: f32, animations: bool) -> [Rect; CARD_COUNT] {
        if streaming != self.streaming {
            self.streaming = streaming;
            self.from = self.previous.filter(|_| animations);
            self.started = time;
        }
        let progress = ((time - self.started) / CARD_MOVE_SECONDS).clamp(0.0, 1.0);
        if progress >= 1.0 {
            self.from = None;
        }
        let eased = 1.0 - (1.0 - progress).powi(3);
        let placed = match self.from {
            Some(from) => std::array::from_fn(|index| {
                Rect::from_min_max(
                    from[index].min.lerp(targets[index].min, eased),
                    from[index].max.lerp(targets[index].max, eased),
                )
            }),
            None => targets,
        };
        self.previous = Some(placed);
        placed
    }

    fn moving(&self) -> bool {
        self.from.is_some()
    }
}

fn right_column(area: Rect, width: f32) -> [Rect; 4] {
    let x = area.right() - width;
    let fixed = MUSIC_HEIGHT + FILES_HEIGHT + SETTINGS_HEIGHT + GAP * 3.0;
    let notices_height = (area.height() - fixed).max(0.0);

    let music = Rect::from_min_size(pos2(x, area.top()), vec2(width, MUSIC_HEIGHT));
    let notices = Rect::from_min_size(pos2(x, music.bottom() + GAP), vec2(width, notices_height));
    let files = Rect::from_min_size(pos2(x, notices.bottom() + GAP), vec2(width, FILES_HEIGHT));
    let settings = Rect::from_min_size(pos2(x, files.bottom() + GAP), vec2(width, SETTINGS_HEIGHT));
    [music, notices, files, settings]
}

/// Reparte la ventana. `phone_ratio` es el ancho del móvil entre su alto, si se
/// está transmitiendo.
fn arrange(area: Rect, phone_ratio: Option<f32>) -> Arrangement {
    let Some(phone_ratio) = phone_ratio else {
        return grid(area).map_or(Arrangement::Single { area }, |cards| Arrangement::Grid { cards });
    };
    if area.width() < COLUMNS_MIN_WIDTH || area.height() < COLUMNS_MIN_HEIGHT {
        return Arrangement::Single { area };
    }

    if phone_ratio > 1.0 {
        let right = right_column(area, SIDE_WIDTH);
        let content_right = area.right() - SIDE_WIDTH - GAP;
        let top = area.bottom() - LANDSCAPE_ROW_HEIGHT;
        let screen = Rect::from_min_max(area.min, pos2(content_right, top - GAP));
        let width = (content_right - area.left() - GAP * 3.0) / 4.0;
        let bottom = std::array::from_fn(|index| {
            let left = area.left() + index as f32 * (width + GAP);
            Rect::from_min_size(pos2(left, top), vec2(width, LANDSCAPE_ROW_HEIGHT))
        });
        return Arrangement::Landscape { screen, bottom, right };
    }

    // Las tarjetas se acercan al móvil: lo que sobra queda en los bordes de la ventana.
    let phone_width = (area.height() - TOOLBAR_HEIGHT) * phone_ratio;
    let side = ((area.width() - phone_width) / 2.0 - CENTER_GUTTER).clamp(SIDE_WIDTH, SIDE_MAX_WIDTH);
    let room = area.width() - side * 2.0 - GAP * 2.0;
    let center_width = room.min(phone_width + (CENTER_GUTTER - GAP) * 2.0);
    let total = side * 2.0 + GAP * 2.0 + center_width;
    let content = Rect::from_center_size(area.center(), vec2(total, area.height()));

    let screen = Rect::from_min_size(
        pos2(content.left() + side + GAP, content.top()),
        vec2(center_width, content.height()),
    );
    let height = (content.height() - GAP * 3.0) / 4.0;
    let left = std::array::from_fn(|index| {
        let top = content.top() + index as f32 * (height + GAP);
        Rect::from_min_size(pos2(content.left(), top), vec2(side, height))
    });
    Arrangement::Columns {
        screen,
        left,
        right: right_column(content, side),
    }
}

/// El rectángulo más grande con las proporciones de `size` que cabe en `area`.
fn fit(area: Rect, size: Vec2) -> Rect {
    let scale = (area.width() / size.x).min(area.height() / size.y);
    Rect::from_center_size(area.center(), size * scale)
}

struct Frame<'a> {
    engine: &'a Engine,
    view: &'a View,
    actions: Option<&'a dyn DesktopActions>,
    time: f32,
    bridge: BridgeState,
    hovering_files: bool,
    recent_files: Vec<RecentFile>,
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    engine: &Engine,
    view: &View,
    texture: Option<&TextureHandle>,
    actions: Option<&dyn DesktopActions>,
    state: &mut DesktopState,
    time: f32,
) {
    let bridge = if matches!(view.status, Status::Connected { .. }) {
        state.link.bridge(view.mirroring, time, view.animations)
    } else {
        state.link.disconnected();
        state.ring_ends = None;
        state.unpair_until = None;
        motion::searching(time, view.animations)
    };
    let hovering_files = ui.input(|input| !input.raw.hovered_files.is_empty());
    let recent_files = actions.map(DesktopActions::recent_files).unwrap_or_default();
    if hovering_files
        || !recent_files.is_empty()
        || actions.is_some_and(|actions| actions.transfer().is_some())
    {
        ui.ctx().request_repaint_after(TRANSFER_REFRESH);
    }
    let frame = Frame {
        engine,
        view,
        actions,
        time,
        bridge,
        hovering_files,
        recent_files,
    };
    let area = ui.max_rect().shrink(OUTER);
    match &view.status {
        Status::Searching => draw_waiting(ui, area, &frame, None),
        Status::Connecting { name } => {
            let text = format!("Conectando con {name}");
            draw_waiting(ui, area, &frame, Some(&text));
        }
        Status::Pairing {
            name,
            code,
            answered,
        } => draw_pairing(ui, area, &frame, name, code, *answered),
        Status::Connected { name } => draw_connected(ui, area, &frame, texture, state, name),
    }
}

fn centered(ui: &mut egui::Ui, rect: Rect, body: impl FnOnce(&mut egui::Ui)) {
    let builder = UiBuilder::new()
        .max_rect(rect)
        .layout(Layout::top_down(Align::Center));
    ui.scope_builder(builder, body);
}

fn draw_wordmark(ui: &mut egui::Ui) {
    dot_figure(ui, "PUENTE", "", 7.0, INK);
}

fn draw_waiting(ui: &mut egui::Ui, area: Rect, frame: &Frame, status: Option<&str>) {
    const CONTENT_WIDTH: f32 = 460.0;
    const CONTENT_HEIGHT: f32 = 430.0;

    let content = Rect::from_center_size(
        area.center(),
        vec2(CONTENT_WIDTH.min(area.width()), CONTENT_HEIGHT.min(area.height())),
    );
    centered(ui, content, |ui| {
        draw_wordmark(ui);
        ui.add_space(28.0);

        let (bridge, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), Sense::hover());
        dots::paint_bridge(ui.painter(), bridge, frame.bridge);
        ui.add_space(20.0);

        label(ui, status.unwrap_or("Esperando al móvil"));
        ui.add_space(20.0);
        if let Some(error) = &frame.view.last_error {
            ui.colored_label(ACCENT, error);
            ui.add_space(12.0);
        }

        if frame.view.paired.is_empty() {
            ui.label("1. Instala Puente en tu móvil Android.");
            ui.label("2. Ábrela con el móvil en esta misma Wi-Fi.");
            ui.label("3. Toca esta PC y compara el código.");
        } else {
            ui.label("Abre Puente en el móvil y se conectará solo.");
            ui.add_space(16.0);
            draw_paired(ui, frame);
        }
    });
}

fn draw_paired(ui: &mut egui::Ui, frame: &Frame) {
    for peer in &frame.view.paired {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(&peer.name).truncate());
            if pill_button(ui, "Olvidar").clicked() {
                let _ = frame.engine.forget(peer.fingerprint);
            }
        });
    }
}

fn draw_pairing(ui: &mut egui::Ui, area: Rect, frame: &Frame, name: &str, code: &str, answered: bool) {
    const PITCH: f32 = 10.0;
    const CONTENT_HEIGHT: f32 = 300.0;

    let content = Rect::from_center_size(area.center(), vec2(area.width(), CONTENT_HEIGHT.min(area.height())));
    centered(ui, content, |ui| {
        label(ui, &format!("Emparejar con {name}"));
        ui.add_space(24.0);
        dot_figure(ui, &spaced_code(code), "", PITCH, INK);
        ui.add_space(24.0);

        if answered {
            let (bridge, _) = ui.allocate_exact_size(vec2(360.0_f32.min(area.width()), 64.0), Sense::hover());
            dots::paint_bridge(ui.painter(), bridge, frame.bridge);
            ui.add_space(12.0);
            label(ui, "Esperando al otro dispositivo");
            return;
        }

        ui.label("Acepta solo si este código es igual en los dos dispositivos.");
        ui.add_space(20.0);
        let width = 280.0;
        let row = Rect::from_center_size(pos2(ui.max_rect().center().x, ui.cursor().top() + 22.0), vec2(width, 44.0));
        ui.scope_builder(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
            if pill(ui, "Coinciden", 44.0, true).clicked() {
                frame.engine.answer_pairing(true);
            }
            if pill(ui, "Cancelar", 44.0, false).clicked() {
                frame.engine.answer_pairing(false);
            }
        });
    });
}

/// El código en dos grupos de tres, más fácil de comparar.
pub(crate) fn spaced_code(code: &str) -> String {
    match code.get(..3).zip(code.get(3..)) {
        Some((first, second)) => format!("{first} {second}"),
        None => code.to_string(),
    }
}

fn draw_connected(
    ui: &mut egui::Ui,
    area: Rect,
    frame: &Frame,
    texture: Option<&TextureHandle>,
    state: &mut DesktopState,
    name: &str,
) {
    send_dropped_files(ui, frame);
    if let Some(ends) = state.ring_ends
        && frame.time >= ends
    {
        state.ring_ends = None;
    }

    let shown = texture.filter(|_| frame.view.mirroring);
    let phone_ratio = shown.map(|texture| {
        let size = texture.size_vec2();
        size.x / size.y
    });
    let toggle_requested = ui.input(|input| input.key_pressed(Key::F2));
    let arrangement = arrange(area, phone_ratio);
    let streaming = shown.is_some();

    let mut cards_visible = true;
    match arrangement {
        Arrangement::Single { area } if !streaming => draw_card_stack(ui, area, frame, state, name, false),
        Arrangement::Single { area } => {
            if toggle_requested {
                state.show_cards = !state.show_cards;
            }
            if state.show_cards {
                draw_card_stack(ui, area, frame, state, name, true);
            } else {
                cards_visible = false;
                if draw_center(ui, area, frame, shown, &mut state.touching, Toggle::Label("Tarjetas")) {
                    state.show_cards = true;
                }
            }
        }
        _ if state.focus && streaming => {
            cards_visible = false;
            if draw_center(ui, area, frame, shown, &mut state.touching, Toggle::Label("Mostrar tarjetas"))
                || toggle_requested
            {
                state.focus = false;
            }
        }
        Arrangement::Columns { screen, .. } | Arrangement::Landscape { screen, .. } => {
            if draw_center(ui, screen, frame, shown, &mut state.touching, Toggle::Icon) || toggle_requested {
                state.focus = true;
            }
        }
        Arrangement::Grid { .. } => {}
    }

    if cards_visible
        && !matches!(arrangement, Arrangement::Single { .. })
        && let Some(targets) = arrangement.card_rects()
    {
        let placed = state.cards.place(targets, streaming, frame.time, frame.view.animations);
        for (index, rect) in placed.into_iter().enumerate() {
            draw_card(ui, index, rect, frame, state, name);
        }
    }

    if frame.hovering_files && !cards_visible {
        draw_drop_overlay(ui, area);
    }
}

fn draw_card(ui: &mut egui::Ui, index: usize, rect: Rect, frame: &Frame, state: &mut DesktopState, name: &str) {
    match index {
        0 => {
            link_card(ui, rect, frame, name);
            unpair_control(ui, rect, frame, state);
        }
        1 => battery_card(ui, rect, frame),
        2 => audio_card(ui, rect, frame),
        3 => ring_card(ui, rect, frame, state),
        4 => music_card(ui, rect, frame),
        5 => notices_card(ui, rect, frame),
        6 => files_card(ui, rect, frame),
        _ => settings_card(ui, rect, frame),
    }
}

fn send_dropped_files(ui: &egui::Ui, frame: &Frame) {
    let dropped = ui.input(|input| input.raw.dropped_files.clone());
    let paths: Vec<PathBuf> = dropped.iter().map(|file| file.path().to_path_buf()).collect();
    if let (false, Some(actions)) = (paths.is_empty(), frame.actions) {
        actions.send_files(paths);
    }
}

fn draw_drop_overlay(ui: &egui::Ui, area: Rect) {
    dots::paint_outline(ui.painter(), area.shrink(4.0), 8.0, 3, ACCENT);
    caption(ui, area.center(), "Suelta para enviar al móvil");
}

fn draw_card_stack(ui: &mut egui::Ui, area: Rect, frame: &Frame, state: &mut DesktopState, name: &str, offer_phone: bool) {
    let toolbar = Rect::from_min_max(pos2(area.left(), area.bottom() - TOOLBAR_HEIGHT), area.max);
    let list = if offer_phone {
        Rect::from_min_max(area.min, pos2(area.right(), toolbar.top() - GAP))
    } else {
        area
    };

    ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            for index in 0..CARD_COUNT {
                let size = vec2(ui.available_width(), STACKED_CARD_HEIGHT);
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                draw_card(ui, index, rect, frame, state, name);
            }
        });
    });

    if offer_phone {
        let builder = UiBuilder::new().max_rect(toolbar).layout(Layout::right_to_left(Align::Center));
        ui.scope_builder(builder, |ui| {
            if pill_button(ui, "Ver móvil").clicked() {
                state.show_cards = false;
            }
        });
    }
}

/// El control para alternar tarjetas: con icono cuando la barra es estrecha.
#[derive(Clone, Copy)]
enum Toggle<'a> {
    Icon,
    Label(&'a str),
}

/// Dibuja el móvil y su barra. Devuelve verdadero si se pulsó el control de `toggle`.
fn draw_center(
    ui: &mut egui::Ui,
    area: Rect,
    frame: &Frame,
    texture: Option<&TextureHandle>,
    touching: &mut bool,
    toggle: Toggle,
) -> bool {
    let toolbar = Rect::from_min_max(pos2(area.left(), area.bottom() - TOOLBAR_HEIGHT), area.max);
    let stage = Rect::from_min_max(area.min, pos2(area.right(), toolbar.top()));

    let screen = texture.and_then(|texture| draw_screen(ui, stage, texture));

    let builder = UiBuilder::new()
        .max_rect(toolbar)
        .layout(Layout::left_to_right(Align::Center));
    ui.scope_builder(builder, |ui| {
        if icon_button(ui, Icon::Stop, 44.0, true)
            .on_hover_text("Cortar la transmisión")
            .clicked()
        {
            frame.engine.send_command(PhoneCommand::StopMirror);
        }
    });

    let builder = UiBuilder::new()
        .max_rect(toolbar)
        .layout(Layout::left_to_right(Align::Center));
    ui.scope_builder(builder, |ui| {
        let controls_width = if frame.view.input_enabled { NAVIGATION_WIDTH } else { 0.0 };
        ui.add_space(((ui.available_width() - controls_width) / 2.0).max(0.0));
        if frame.view.input_enabled {
            draw_navigation(ui, frame.engine);
        } else {
            label(ui, "Activa el control en el móvil");
        }
    });

    let builder = UiBuilder::new()
        .max_rect(toolbar)
        .layout(Layout::right_to_left(Align::Center));
    let toggled = ui
        .scope_builder(builder, |ui| match toggle {
            Toggle::Icon => icon_button(ui, Icon::Panels, 44.0, false)
                .on_hover_text("Ocultar tarjetas (F2)")
                .clicked(),
            Toggle::Label(text) => pill_button(ui, text).on_hover_text("Atajo: F2").clicked(),
        })
        .inner;

    if let Some(screen) = screen.filter(|_| frame.view.input_enabled) {
        forward_input(ui, screen, frame.engine, touching);
    }
    toggled
}

fn draw_screen(ui: &mut egui::Ui, stage: Rect, texture: &TextureHandle) -> Option<Rect> {
    let rect = fit(stage, texture.size_vec2());
    ui.allocate_rect(rect, Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::Default);
    egui::Image::new((texture.id(), rect.size()))
        .corner_radius(SCREEN_RADIUS)
        .paint_at(ui, rect);
    ui.painter().rect_stroke(
        rect,
        SCREEN_RADIUS,
        Stroke::new(1.0, EDGE),
        StrokeKind::Outside,
    );
    Some(rect)
}

fn draw_navigation(ui: &mut egui::Ui, engine: &Engine) {
    ui.spacing_mut().item_spacing.x = 28.0;
    let buttons = [
        (Icon::Back, NavAction::Back, "Atrás"),
        (Icon::Home, NavAction::Home, "Inicio"),
        (Icon::Recents, NavAction::Recents, "Recientes"),
    ];
    for (icon, action, hint) in buttons {
        if icon_button(ui, icon, 44.0, false).on_hover_text(hint).clicked() {
            engine.send_input(Input::Navigate { action });
        }
    }
}

fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "—".to_string(), |value| value.to_string())
}

fn link_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame, name: &str) {
    const STATS_HEIGHT: f32 = 46.0;
    const NAME_MIN_HEIGHT: f32 = 130.0;
    const HINT_MIN_HEIGHT: f32 = 20.0;
    let view = frame.view;

    card_full(ui, rect, "01", "Enlace", true, "", |ui| {
        // En tarjetas bajas el nombre cede su sitio para que las cifras quepan enteras.
        let show_name = ui.available_height() >= NAME_MIN_HEIGHT;
        let reserved = STATS_HEIGHT + if show_name { 30.0 } else { 8.0 };
        let bridge_height = (ui.available_height() - reserved).clamp(26.0, 110.0);
        let (bridge, _) = ui.allocate_exact_size(vec2(ui.available_width(), bridge_height), Sense::hover());
        dots::paint_bridge(ui.painter(), bridge, frame.bridge);

        ui.add_space(6.0);
        if show_name {
            ui.horizontal(|ui| {
                label(ui, "PC");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(egui::Label::new(parts::label_text(name)).truncate());
                });
            });
            ui.add_space(6.0);
        }
        ui.horizontal(|ui| {
            let figure = view.round_trip_ms.map_or_else(|| "-".to_string(), |value| value.min(9999).to_string());
            dot_figure(ui, &figure, "ms", dots::MIN_TEXT_PITCH, INK);
            ui.columns(2, |columns| {
                stat(&mut columns[0], "fps", &optional(view.video_fps));
                let mbps = view.video_mbps.map(|mbps| format!("{mbps:.1}"));
                stat(&mut columns[1], "mbps", &optional(mbps));
            });
        });
        if !view.mirroring && ui.available_height() >= HINT_MIN_HEIGHT {
            ui.add_space(6.0);
            label(ui, "Comparte la pantalla desde el móvil");
        }
    });
}

/// Desvincular pide un segundo toque: deshacerlo obliga a emparejar otra vez.
fn unpair_control(ui: &mut egui::Ui, card: Rect, frame: &Frame, state: &mut DesktopState) {
    const CONFIRM_SECONDS: f32 = 4.0;
    const HEADER_HEIGHT: f32 = 28.0;
    /// Por debajo de este ancho, el texto chocaría con el título de la tarjeta.
    const TEXT_MIN_WIDTH: f32 = 300.0;

    let confirming = state.unpair_until.is_some_and(|until| frame.time < until);
    let header = Rect::from_min_size(
        card.min + vec2(PADDING, PADDING - 8.0),
        vec2(card.width() - PADDING * 2.0, HEADER_HEIGHT),
    );
    let builder = UiBuilder::new()
        .max_rect(header)
        .layout(Layout::right_to_left(Align::Center));
    ui.scope_builder(builder, |ui| {
        if confirming {
            ui.ctx().request_repaint_after(TRANSFER_REFRESH);
            if pill(ui, "¿Seguro?", HEADER_HEIGHT, true).clicked() {
                state.unpair_until = None;
                let _ = frame.engine.unpair();
            }
        } else {
            let pressed = if card.width() >= TEXT_MIN_WIDTH {
                pill(ui, "Desvincular", HEADER_HEIGHT, false).clicked()
            } else {
                icon_button(ui, Icon::Unlink, HEADER_HEIGHT, false)
                    .on_hover_text("Desvincular este móvil")
                    .clicked()
            };
            if pressed {
                state.unpair_until = Some(frame.time + CONFIRM_SECONDS);
            }
        }
    });
}

fn battery_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame) {
    const RING_DOTS: usize = 54;
    const LOW_PERCENT: u8 = 15;
    const CAPTION_HEIGHT: f32 = 20.0;
    const RING_SWEEP_SECONDS: f32 = 0.6;
    const CHARGE_CYCLE_SECONDS: f32 = 2.4;
    const PITCH_PER_RADIUS: f32 = 0.072;
    const MAX_FIGURE_PITCH: f32 = 12.0;

    card_full(ui, rect, "02", "Batería", false, "", |ui| {
        let (space, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let ring_area = Rect::from_min_max(space.min, pos2(space.right(), space.bottom() - CAPTION_HEIGHT));
        let radius = (ring_area.height().min(ring_area.width()) / 2.0 - 5.0).max(12.0);
        let (percent, charging) = frame.view.battery.unwrap_or((0, false));
        let known = frame.view.battery.is_some();

        // Con poco alto, la cifra va al lado del anillo para que quepa.
        let beside = radius < 50.0;
        let center = if beside {
            pos2(ring_area.left() + radius + 5.0, ring_area.center().y)
        } else {
            ring_area.center()
        };
        let target = f32::from(percent) / 100.0;
        let shown = if frame.view.animations {
            ui.ctx().animate_value_with_time(ui.scope_id().with("bateria"), target, RING_SWEEP_SECONDS)
        } else {
            target
        };
        if charging {
            let phase = frame.view.animations.then(|| (frame.time / CHARGE_CYCLE_SECONDS).fract());
            dots::paint_charging_ring(ui.painter(), center, radius, RING_DOTS, shown, phase);
        } else {
            dots::paint_ring(ui.painter(), center, radius, RING_DOTS, shown, known);
        }

        let text = if known { percent.to_string() } else { "-".to_string() };
        let pitch = (radius * PITCH_PER_RADIUS).clamp(dots::MIN_TEXT_PITCH, MAX_FIGURE_PITCH);
        let size = dots::text_size(&text, pitch);
        let color = if known && percent <= LOW_PERCENT && !charging { ACCENT } else { INK };
        let origin = if beside {
            pos2(center.x + radius + 16.0, center.y - size.y / 2.0)
        } else {
            center - size / 2.0
        };
        dots::paint_text(ui.painter(), origin, &text, pitch, color);

        if charging {
            ui.painter().text(
                pos2(space.center().x, space.bottom() - CAPTION_HEIGHT / 2.0),
                egui::Align2::CENTER_CENTER,
                "CARGANDO",
                egui::FontId::new(TEXT_LABEL, crate::theme::medium()),
                LOADING,
            );
        }
    });
}

fn caption(ui: &egui::Ui, position: Pos2, text: &str) {
    ui.painter().text(
        position,
        egui::Align2::CENTER_CENTER,
        text.to_uppercase(),
        egui::FontId::new(TEXT_LABEL, crate::theme::medium()),
        INK_SOFT,
    );
}

fn audio_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame) {
    let levels = frame
        .actions
        .map(DesktopActions::audio_levels)
        .unwrap_or_default();
    let enabled = frame.view.audio_enabled;
    let sounding = enabled && levels.iter().any(|level| *level > SILENCE_LEVEL);
    const SWITCH_HEIGHT: f32 = 28.0;

    card_full(ui, rect, "03", "Audio", sounding, "", |ui| {
        let height = (ui.available_height() - SWITCH_HEIGHT - 6.0).max(24.0);
        let (wave, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let mut columns = vec![0.0; WAVE_COLUMNS.saturating_sub(levels.len())];
        if enabled {
            columns.extend(&levels);
        } else {
            columns.resize(WAVE_COLUMNS, 0.0);
        }
        dots::paint_wave(ui.painter(), wave, &columns, 6.0);
        ui.add_space(6.0);

        let status = match (enabled, sounding) {
            (false, _) => "Desactivado",
            (true, true) => "Sonando en la PC",
            (true, false) => "Sin audio",
        };
        if switch_row(ui, status, enabled, SWITCH_HEIGHT).clicked() {
            frame.engine.set_audio_enabled(!enabled);
        }
    });
}

fn music_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame) {
    let media = frame.view.media.as_ref();
    let send = |action| frame.engine.send_command(PhoneCommand::Media { action });

    card_full(ui, rect, "05", "Música", false, "", |ui| {
        let Some(media) = media else {
            label(ui, "Nada sonando");
            return;
        };
        let title = displayable(ui, &media.title);
        let title = if title.is_empty() { "Sin título".to_string() } else { title };
        ui.add(egui::Label::new(RichText::new(title).size(14.0)).truncate());
        let artist = displayable(ui, &media.artist);
        ui.add(egui::Label::new(RichText::new(artist).color(INK_SOFT)).truncate());
        ui.add_space(6.0);
        ui.horizontal_centered(|ui| {
            ui.add_space(((ui.available_width() - 40.0 * 2.0 - 52.0 - GAP * 2.0) / 2.0).max(0.0));
            if icon_button(ui, Icon::Previous, 40.0, false).on_hover_text("Anterior").clicked() {
                send(MediaAction::Previous);
            }
            let icon = if media.playing { Icon::Pause } else { Icon::Play };
            let hint = if media.playing { "Pausa" } else { "Reproducir" };
            if icon_button(ui, icon, 52.0, true).on_hover_text(hint).clicked() {
                send(MediaAction::PlayPause);
            }
            if icon_button(ui, Icon::Next, 40.0, false).on_hover_text("Siguiente").clicked() {
                send(MediaAction::Next);
            }
        });
    });
}

fn notices_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame) {
    let notices = &frame.view.notifications;
    let count = if notices.is_empty() { String::new() } else { notices.len().to_string() };

    card_full(ui, rect, "06", "Avisos", !notices.is_empty(), &count, |ui| {
        if notices.is_empty() {
            label(ui, "Sin avisos");
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("avisos")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for info in notices {
                    draw_notice(ui, frame.engine, info);
                    ui.add_space(4.0);
                }
            });
    });
}

fn draw_notice(ui: &mut egui::Ui, engine: &Engine, info: &NotificationInfo) {
    const CLOSE_SIZE: f32 = 28.0;
    const BODY_CHARACTERS: usize = 110;

    ui.horizontal_top(|ui| {
        let width = (ui.available_width() - CLOSE_SIZE - GAP).max(40.0);
        ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            // Con ancho fijo, la X queda alineada aunque el texto sea corto.
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 2.0;
            let app = displayable(ui, &info.app).to_uppercase();
            ui.add(egui::Label::new(RichText::new(app).color(INK_SOFT)).truncate().selectable(false));
            let title = displayable(ui, &info.title);
            if !title.is_empty() {
                ui.add(egui::Label::new(RichText::new(title).color(INK)).truncate());
            }
            let body: String = displayable(ui, &info.text).chars().take(BODY_CHARACTERS).collect();
            if !body.is_empty() {
                ui.label(RichText::new(body).color(INK_SOFT));
            }
        });
        if icon_button(ui, Icon::Close, CLOSE_SIZE, false).on_hover_text("Descartar").clicked() {
            engine.send_command(PhoneCommand::Dismiss {
                key: info.key.clone(),
            });
        }
    });
}

fn ring_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame, state: &mut DesktopState) {
    const BUTTON: f32 = 44.0;
    const CAPTION_HEIGHT: f32 = 20.0;
    const RINGS: usize = 3;

    let remaining = state.ring_ends.map(|ends| (ends - frame.time).ceil().max(0.0));

    card_full(ui, rect, "04", "Buscar móvil", remaining.is_some(), "", |ui| {
        let (space, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let room = Rect::from_min_max(space.min, pos2(space.right(), space.bottom() - CAPTION_HEIGHT));
        let outer = (room.height().min(room.width()) / 2.0 - 2.0).max(BUTTON / 2.0 + 4.0);

        let pulse = remaining
            .filter(|_| frame.view.animations)
            .map(|_| (frame.time * 0.9).fract());
        dots::paint_radar(ui.painter(), room.center(), BUTTON / 2.0 + 6.0, outer, RINGS, pulse);

        let button = Rect::from_center_size(room.center(), Vec2::splat(BUTTON));
        let icon = if remaining.is_some() { Icon::Stop } else { Icon::Bell };
        let hint = if remaining.is_some() { "Parar" } else { "Hacer sonar el móvil" };
        if icon_button_at(ui, button, icon, true).on_hover_text(hint).clicked() {
            let on = remaining.is_none();
            state.ring_ends = on.then_some(frame.time + RING_SECONDS);
            frame.engine.send_command(PhoneCommand::Ring { on });
        }

        let text = match remaining {
            Some(seconds) => format!("Sonando {seconds:.0} s"),
            None => "Hacer sonar".to_string(),
        };
        caption(ui, pos2(space.center().x, space.bottom() - CAPTION_HEIGHT / 2.0), &text);
    });
}

fn files_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame) {
    const STATUS_HEIGHT: f32 = 20.0;
    const ZONE_INSET: f32 = 12.0;
    const BUTTON_HEIGHT: f32 = 32.0;

    let transfer = frame.actions.and_then(DesktopActions::transfer);

    card_full(ui, rect, "07", "Archivos", transfer.is_some(), "", |ui| {
        let (space, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let zone = Rect::from_min_max(space.min, pos2(space.right(), space.bottom() - STATUS_HEIGHT));
        let status = Rect::from_min_max(pos2(space.left(), zone.bottom() + 4.0), space.max);

        let color = if frame.hovering_files { ACCENT } else { crate::theme::DOT_OFF };
        dots::paint_outline(ui.painter(), zone, 6.0, 2, color);

        let has_list = !frame.recent_files.is_empty() && !frame.hovering_files;
        let button_center = if has_list {
            pos2(zone.center().x, zone.bottom() - ZONE_INSET - BUTTON_HEIGHT / 2.0)
        } else {
            zone.center() + vec2(0.0, 12.0)
        };
        let button_rect = Rect::from_center_size(button_center, vec2(zone.width(), BUTTON_HEIGHT));

        if frame.hovering_files {
            caption(ui, zone.center(), "Suelta para enviar");
        } else {
            if has_list {
                let list = Rect::from_min_max(
                    zone.min + Vec2::splat(ZONE_INSET),
                    pos2(zone.right() - ZONE_INSET, button_rect.top() - 4.0),
                );
                draw_recent_files(ui, list, &frame.recent_files);
            } else {
                caption(ui, zone.center() - vec2(0.0, 16.0), "Arrastra archivos aquí");
            }
            let builder = UiBuilder::new()
                .max_rect(button_rect)
                .layout(Layout::top_down(Align::Center));
            ui.scope_builder(builder, |ui| {
                if let Some(actions) = frame.actions
                    && pill_button(ui, "Elegir").clicked()
                {
                    actions.pick_and_send_file();
                }
            });
        }

        match transfer {
            Some(transfer) => {
                let bar = Rect::from_min_size(status.left_top() + vec2(3.0, 0.0), vec2(status.width() - 6.0, status.height()));
                dots::paint_progress(ui.painter(), bar, 6.0, transfer.fraction);
            }
            None => {
                if let Some(last_file) = &frame.view.last_file {
                    let builder = UiBuilder::new().max_rect(status);
                    ui.scope_builder(builder, |ui| {
                        ui.add(egui::Label::new(RichText::new(last_file).color(INK_SOFT)).truncate());
                    });
                }
            }
        }
    });
}

/// Filas que caben, reservando una para avisar de las que no se ven.
fn visible_rows(room: f32, total: usize) -> (usize, usize) {
    let fit = ((room / RECENT_ROW_HEIGHT).floor() as usize).max(1);
    if total <= fit {
        (total, 0)
    } else {
        let shown = fit - 1;
        (shown, total - shown)
    }
}

pub(crate) fn draw_recent_files(ui: &mut egui::Ui, area: Rect, files: &[RecentFile]) {
    const ICON: f32 = 14.0;

    let (shown, hidden) = visible_rows(area.height(), files.len());
    let painter = ui.painter().with_clip_rect(area);
    let font = egui::FontId::new(TEXT_LABEL, crate::theme::medium());

    for (row, file) in files.iter().take(shown).enumerate() {
        let center_y = area.top() + (row as f32 + 0.5) * RECENT_ROW_HEIGHT;
        let icon_rect = Rect::from_center_size(pos2(area.left() + ICON / 2.0, center_y), Vec2::splat(ICON));
        let (icon, color) = if file.delivered { (Icon::Check, INK) } else { (Icon::Close, ACCENT) };
        icons::paint(&painter, icon_rect, icon, color.gamma_multiply(file.opacity));

        let room = area.width() - ICON - 8.0;
        let text = if file.delivered { file.name.clone() } else { format!("No se pudo: {}", file.name) };
        let mut job = egui::text::LayoutJob::simple_singleline(text, font.clone(), INK.gamma_multiply(file.opacity));
        job.wrap = egui::text::TextWrapping::truncate_at_width(room);
        let galley = painter.layout_job(job);
        painter.galley(
            pos2(icon_rect.right() + 8.0, center_y - galley.size().y / 2.0),
            galley,
            INK,
        );
    }

    if hidden > 0 {
        let center_y = area.top() + (shown as f32 + 0.5) * RECENT_ROW_HEIGHT;
        painter.text(
            pos2(area.left() + ICON + 8.0, center_y),
            egui::Align2::LEFT_CENTER,
            if shown == 0 { format!("{hidden} archivos") } else { format!("Y {hidden} más") },
            font,
            INK_SOFT,
        );
    }
}

fn settings_card(ui: &mut egui::Ui, rect: Rect, frame: &Frame) {
    const ROW: f32 = 30.0;

    card_full(ui, rect, "08", "Ajustes", false, "", |ui| {
        if let Some(actions) = frame.actions {
            let on = actions.clipboard_sync();
            if switch_row(ui, "Portapapeles", on, ROW).clicked() {
                actions.set_clipboard_sync(!on);
            }
        }
        let animations = frame.view.animations;
        if switch_row(ui, "Animaciones", animations, ROW).clicked() {
            frame.engine.set_animations(!animations);
        }
    });
}

fn normalized(screen: Rect, position: Pos2) -> (f32, f32) {
    let x = (position.x - screen.min.x) / screen.width();
    let y = (position.y - screen.min.y) / screen.height();
    (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))
}

fn wheel_lines(unit: egui::MouseWheelUnit, delta: f32) -> f32 {
    match unit {
        egui::MouseWheelUnit::Line => delta,
        egui::MouseWheelUnit::Point => delta / 40.0,
        egui::MouseWheelUnit::Page => delta * 10.0,
    }
}

fn forward_input(ui: &egui::Ui, screen: Rect, engine: &Engine, touching: &mut bool) {
    let (events, hover) = ui.input(|input| (input.events.clone(), input.pointer.hover_pos()));
    let touch = |phase, position| {
        let (x, y) = normalized(screen, position);
        engine.send_input(Input::Touch { phase, x, y });
    };

    for event in events {
        match event {
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                ..
            } => {
                if pressed && screen.contains(pos) {
                    *touching = true;
                    touch(TouchPhase::Down, pos);
                } else if !pressed && *touching {
                    *touching = false;
                    touch(TouchPhase::Up, pos);
                }
            }
            Event::PointerMoved(position) if *touching => touch(TouchPhase::Move, position),
            Event::MouseWheel { unit, delta, .. } => {
                if let Some(position) = hover.filter(|position| screen.contains(*position)) {
                    let (x, y) = normalized(screen, position);
                    let lines = wheel_lines(unit, delta.y);
                    engine.send_input(Input::Scroll { x, y, lines });
                }
            }
            Event::Text(text) | Event::Paste(text) => engine.send_input(Input::Text { text }),
            Event::Key {
                key,
                pressed: true,
                ..
            } => {
                let key = match key {
                    Key::Backspace => KeyPress::Backspace,
                    Key::Enter => KeyPress::Enter,
                    _ => continue,
                };
                engine.send_input(Input::Key { key });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(width: f32, height: f32) -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(width, height))
    }

    fn assert_inside_and_apart(area: Rect, rects: &[Rect]) {
        for (index, rect) in rects.iter().enumerate() {
            assert!(area.expand(0.5).contains_rect(*rect), "fuera del área: {rect:?}");
            for other in &rects[index + 1..] {
                assert!(!rect.shrink(0.5).intersects(*other), "se solapan: {rect:?} y {other:?}");
            }
        }
    }

    const PORTRAIT: Option<f32> = Some(0.47);
    const LANDSCAPE: Option<f32> = Some(2.14);

    #[test]
    fn portrait_columns_fit_without_overlapping() {
        for (width, height) in [(1100.0, 860.0), (1920.0, 1080.0), (1165.0, 625.0), (860.0, 700.0), (2560.0, 1400.0)] {
            let area = window(width, height);
            let Arrangement::Columns { screen, left, right } = arrange(area, PORTRAIT) else {
                panic!("se esperaban columnas en {width}x{height}");
            };
            let mut all = vec![screen];
            all.extend(left);
            all.extend(right);
            assert_inside_and_apart(area, &all);
            assert!((left[3].bottom() - area.bottom()).abs() < 0.5);
            assert!((right[3].bottom() - area.bottom()).abs() < 0.5);
        }
    }

    #[test]
    fn the_cards_stay_close_to_the_phone() {
        let area = window(1920.0, 1080.0);
        let Arrangement::Columns { screen, left, .. } = arrange(area, PORTRAIT) else {
            panic!("se esperaban columnas");
        };
        let phone = fit(
            Rect::from_min_max(screen.min, pos2(screen.right(), screen.bottom() - TOOLBAR_HEIGHT)),
            vec2(0.47, 1.0),
        );
        assert!(phone.left() - left[0].right() <= CENTER_GUTTER + 1.0);
    }

    #[test]
    fn a_landscape_phone_keeps_every_card_around_it() {
        let area = window(1400.0, 900.0);
        let Arrangement::Landscape { screen, bottom, right } = arrange(area, LANDSCAPE) else {
            panic!("se esperaba la disposición horizontal");
        };
        assert!(screen.width() > screen.height());
        let mut all = vec![screen];
        all.extend(bottom);
        all.extend(right);
        assert_inside_and_apart(area, &all);
    }

    #[test]
    fn without_a_stream_the_cards_fill_the_window() {
        for (width, height) in [(1235.0, 1386.0), (1600.0, 700.0), (560.0, 800.0)] {
            let area = window(width, height);
            let Arrangement::Grid { cards } = arrange(area, None) else {
                panic!("se esperaba la cuadrícula en {width}x{height}");
            };
            assert_inside_and_apart(area, &cards);
            let bottom = cards.iter().map(|card| card.bottom()).fold(0.0, f32::max);
            let right = cards.iter().map(|card| card.right()).fold(0.0, f32::max);
            assert!((bottom - area.bottom()).abs() < 0.5);
            assert!((right - area.right()).abs() < 0.5);
        }
    }

    #[test]
    fn small_windows_show_one_thing_at_a_time() {
        let narrow = window(640.0, 800.0);
        assert_eq!(arrange(narrow, PORTRAIT), Arrangement::Single { area: narrow });
        assert_eq!(arrange(narrow, LANDSCAPE), Arrangement::Single { area: narrow });
        let tiny = window(400.0, 300.0);
        assert_eq!(arrange(tiny, None), Arrangement::Single { area: tiny });
    }

    #[test]
    fn cards_glide_only_when_the_stream_starts_or_stops() {
        let area = window(1400.0, 900.0);
        let first = arrange(area, None).card_rects().unwrap();
        let second = arrange(area, PORTRAIT).card_rects().unwrap();
        let mut motion = CardMotion::default();

        assert_eq!(motion.place(first, false, 1.0, true), first);
        assert!(!motion.moving());

        let midway = motion.place(second, true, 2.0, true);
        assert_eq!(midway, first, "empieza donde estaban");
        assert!(motion.moving());
        assert_ne!(motion.place(second, true, 2.1, true), second);

        assert_eq!(motion.place(second, true, 3.0, true), second);
        assert!(!motion.moving());
    }

    #[test]
    fn without_animations_the_cards_jump() {
        let area = window(1400.0, 900.0);
        let first = arrange(area, None).card_rects().unwrap();
        let second = arrange(area, PORTRAIT).card_rects().unwrap();
        let mut motion = CardMotion::default();
        motion.place(first, false, 1.0, false);
        assert_eq!(motion.place(second, true, 2.0, false), second);
    }

    #[test]
    fn the_list_says_how_many_rows_do_not_fit() {
        assert_eq!(visible_rows(RECENT_ROW_HEIGHT * 3.0, 2), (2, 0));
        assert_eq!(visible_rows(RECENT_ROW_HEIGHT * 3.0, 3), (3, 0));
        assert_eq!(visible_rows(RECENT_ROW_HEIGHT * 3.0, 7), (2, 5));
        assert_eq!(visible_rows(5.0, 4), (0, 4));
    }

    #[test]
    fn the_screen_keeps_its_proportions() {
        let rect = fit(window(600.0, 900.0), vec2(1080.0, 2316.0));
        let ratio = rect.width() / rect.height();
        assert!((ratio - 1080.0 / 2316.0).abs() < 0.001);
        assert!(rect.height() <= 900.5);
    }

    #[test]
    fn codes_are_grouped_in_threes() {
        assert_eq!(spaced_code("482917"), "482 917");
    }
}
