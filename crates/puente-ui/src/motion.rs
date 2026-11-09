//! El movimiento de la app: el puente que se tiende y se cierra.

use crate::dots::BridgeState;

const CLOSE_SECONDS: f32 = 0.9;
/// Al conectar se salta a la parte final: el puente listo y la onda que lo recorre.
const CLOSE_FROM: f32 = 0.62;
const CLOSE_TO: f32 = 0.9;
const SEARCH_SECONDS: f32 = 4.2;
const PACKET_SPEED: f32 = 0.45;
/// Cuánto se ve tendido el puente mientras busca, sin animaciones.
const STILL_REACH: f32 = 0.6;

/// El puente mientras busca a la otra orilla.
pub fn searching(time: f32, animations: bool) -> BridgeState {
    if !animations {
        return BridgeState::Reaching { reach: STILL_REACH };
    }
    BridgeState::Building {
        progress: (time / SEARCH_SECONDS).fract(),
    }
}

/// Recuerda cuándo se conectó para cerrar el puente una sola vez.
#[derive(Default)]
pub struct LinkMotion {
    linked_at: Option<f32>,
}

impl LinkMotion {
    pub fn disconnected(&mut self) {
        self.linked_at = None;
    }

    pub fn bridge(&mut self, mirroring: bool, time: f32, animations: bool) -> BridgeState {
        let linked_at = *self.linked_at.get_or_insert(time);
        let progress = (time - linked_at) / CLOSE_SECONDS;
        if animations && progress < 1.0 {
            let eased = 1.0 - (1.0 - progress.max(0.0)).powi(3);
            return BridgeState::Building {
                progress: CLOSE_FROM + (CLOSE_TO - CLOSE_FROM) * eased,
            };
        }
        if mirroring && animations {
            BridgeState::Traffic {
                position: (time * PACKET_SPEED).fract(),
            }
        } else {
            BridgeState::Linked
        }
    }

    /// Verdadero mientras el puente se está cerrando.
    pub fn settling(&self, time: f32) -> bool {
        self.linked_at
            .is_some_and(|linked_at| time - linked_at < CLOSE_SECONDS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bridge_closes_once_and_then_stays_linked() {
        let mut motion = LinkMotion::default();
        assert!(matches!(motion.bridge(false, 10.0, true), BridgeState::Building { .. }));
        assert!(motion.settling(10.1));
        assert_eq!(motion.bridge(false, 11.0, true), BridgeState::Linked);
        assert!(!motion.settling(11.0));
    }

    #[test]
    fn it_closes_again_after_a_disconnection() {
        let mut motion = LinkMotion::default();
        motion.bridge(false, 10.0, true);
        motion.disconnected();
        assert!(matches!(motion.bridge(false, 50.0, true), BridgeState::Building { .. }));
    }

    #[test]
    fn without_animations_nothing_moves() {
        let mut motion = LinkMotion::default();
        assert_eq!(motion.bridge(true, 10.0, false), BridgeState::Linked);
        assert_eq!(searching(1.0, false), searching(2.0, false));
    }
}
