//! El enlace que la PC enseña como código QR para que el móvil sepa dónde buscarla.
//! Solo lleva la dirección: la identidad se comprueba después, al emparejar.

use std::net::IpAddr;

const PREFIX: &str = "puente://connect?ip=";

pub fn connect_link(ip: IpAddr) -> String {
    format!("{PREFIX}{ip}")
}

/// `None` si el texto no es un enlace de Puente con una dirección válida.
pub fn parse_connect_link(text: &str) -> Option<IpAddr> {
    let rest = text.trim().strip_prefix(PREFIX)?;
    rest.split('&').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_carries_the_address_back_and_forth() {
        let ip: IpAddr = "192.168.1.20".parse().unwrap();
        assert_eq!(connect_link(ip), "puente://connect?ip=192.168.1.20");
        assert_eq!(parse_connect_link(&connect_link(ip)), Some(ip));
        assert_eq!(parse_connect_link(" puente://connect?ip=10.0.0.7&v=2\n"), Some("10.0.0.7".parse().unwrap()));
    }

    #[test]
    fn other_codes_are_not_taken_for_an_address() {
        for text in ["", "https://example.com", "puente://connect?ip=", "puente://connect?ip=hola", "192.168.1.20"] {
            assert_eq!(parse_connect_link(text), None, "{text}");
        }
    }
}
