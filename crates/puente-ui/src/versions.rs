//! Aviso de que la PC y el móvil tienen versiones distintas.

use std::cmp::Ordering;

use puente_core::APP_VERSION;

/// Lo que dice el aviso. `peer` vacía significa anterior a la 0.3.1, que no la envía.
#[derive(Clone, Debug, PartialEq)]
pub struct Mismatch {
    pub own: &'static str,
    pub peer: String,
    /// Hay que actualizar el otro lado; si no, este.
    pub peer_is_older: bool,
}

/// `None` si no hay conexión o las dos versiones coinciden.
pub fn mismatch(peer_version: Option<&str>) -> Option<Mismatch> {
    mismatch_between(APP_VERSION, peer_version?)
}

fn mismatch_between(own: &'static str, peer: &str) -> Option<Mismatch> {
    if peer == own {
        return None;
    }
    let peer_is_older = peer.is_empty() || compare(peer, own) == Ordering::Less;
    Some(Mismatch {
        own,
        peer: peer.to_string(),
        peer_is_older,
    })
}

/// Compara número a número: 0.10.0 es más nueva que 0.9.9.
fn compare(a: &str, b: &str) -> Ordering {
    let parts = |text: &str| -> Vec<u64> { text.split('.').map(|part| part.parse().unwrap_or(0)).collect() };
    parts(a).cmp(&parts(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_version_shows_nothing() {
        assert_eq!(mismatch_between("0.3.1", "0.3.1"), None);
        assert_eq!(mismatch(None), None);
    }

    #[test]
    fn the_older_side_is_the_one_to_update() {
        assert!(mismatch_between("0.3.1", "0.3.0").unwrap().peer_is_older);
        assert!(!mismatch_between("0.3.1", "0.4.0").unwrap().peer_is_older);
        assert!(!mismatch_between("0.9.0", "0.10.0").unwrap().peer_is_older);
    }

    #[test]
    fn a_peer_that_does_not_say_its_version_is_older() {
        let notice = mismatch_between("0.3.1", "").unwrap();
        assert!(notice.peer_is_older);
        assert!(notice.peer.is_empty());
    }
}
