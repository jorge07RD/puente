//! El enlace que la PC enseña como código QR para que el móvil sepa dónde buscarla.
//! Solo lleva direcciones: la identidad se comprueba después, al emparejar.

use std::net::IpAddr;

const PREFIX: &str = "puente://connect?ip=";

/// Lleva todas las direcciones de la PC: el móvil prueba cada una, porque no sabe por
/// cuál de sus redes (Wi-Fi, VPN) la va a alcanzar.
pub fn connect_link(addresses: &[IpAddr]) -> String {
    let list: Vec<String> = addresses.iter().map(IpAddr::to_string).collect();
    format!("{PREFIX}{}", list.join(","))
}

/// Vacío si el texto no es un enlace de Puente con alguna dirección válida.
pub fn parse_connect_link(text: &str) -> Vec<IpAddr> {
    let Some(rest) = text.trim().strip_prefix(PREFIX) else {
        return Vec::new();
    };
    let list = rest.split('&').next().unwrap_or_default();
    list.split(',').filter_map(|address| address.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ips(list: &[&str]) -> Vec<IpAddr> {
        list.iter().map(|ip| ip.parse().unwrap()).collect()
    }

    #[test]
    fn a_link_carries_the_addresses_back_and_forth() {
        let both = ips(&["192.168.1.20", "100.64.0.7"]);
        assert_eq!(connect_link(&both), "puente://connect?ip=192.168.1.20,100.64.0.7");
        assert_eq!(parse_connect_link(&connect_link(&both)), both);
        assert_eq!(parse_connect_link(" puente://connect?ip=10.0.0.7&v=2\n"), ips(&["10.0.0.7"]));
    }

    #[test]
    fn other_codes_are_not_taken_for_an_address() {
        for text in ["", "https://example.com", "puente://connect?ip=", "puente://connect?ip=hola", "192.168.1.20"] {
            assert!(parse_connect_link(text).is_empty(), "{text}");
        }
    }
}
