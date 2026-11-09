//! TLS 1.3 mutuo con certificados autofirmados.
//!
//! No hay autoridad certificadora, así que los verificadores aceptan cualquier
//! certificado y solo comprueban que el otro lado posee su clave privada. La
//! identidad se decide después: el llamador compara `Secure::peer` con las
//! huellas emparejadas.

use std::net::SocketAddr;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, ring, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::version::TLS13;
use rustls::{ClientConfig, DigitallySignedStruct, DistinguishedName, ServerConfig, SignatureScheme};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

use crate::{Error, Fingerprint, Identity, Result};

const SERVER_NAME: &str = "puente.local";

#[derive(Debug)]
struct ProofOfKeyOnly(Arc<CryptoProvider>);

impl ProofOfKeyOnly {
    fn verify_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            certificate,
            signature,
            &self.0.signature_verification_algorithms,
        )
    }

    fn schemes(&self) -> Vec<SignatureScheme> {
        self.0
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn tls12_refused() -> rustls::Error {
    rustls::Error::General("TLS 1.2 no está permitido".to_string())
}

impl ServerCertVerifier for ProofOfKeyOnly {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _certificate: &CertificateDer<'_>,
        _signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Err(tls12_refused())
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.verify_signature(message, certificate, signature)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.schemes()
    }
}

impl ClientCertVerifier for ProofOfKeyOnly {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _certificate: &CertificateDer<'_>,
        _signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Err(tls12_refused())
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.verify_signature(message, certificate, signature)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.schemes()
    }
}

/// Configuración TLS de un dispositivo, para llamar y para aceptar llamadas.
pub struct TlsContext {
    connector: TlsConnector,
    acceptor: TlsAcceptor,
}

impl TlsContext {
    pub fn new(identity: &Identity) -> Result<Self> {
        let provider = Arc::new(ring::default_provider());
        let verifier = Arc::new(ProofOfKeyOnly(provider.clone()));

        let client = ClientConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&TLS13])?
            .dangerous()
            .with_custom_certificate_verifier(verifier.clone())
            .with_client_auth_cert(vec![identity.certificate()], identity.private_key())?;

        let server = ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&TLS13])?
            .with_client_cert_verifier(verifier)
            .with_single_cert(vec![identity.certificate()], identity.private_key())?;

        Ok(Self {
            connector: TlsConnector::from(Arc::new(client)),
            acceptor: TlsAcceptor::from(Arc::new(server)),
        })
    }

    pub async fn connect(&self, address: SocketAddr) -> Result<Secure> {
        let tcp = TcpStream::connect(address).await?;
        tcp.set_nodelay(true)?;
        let name = ServerName::try_from(SERVER_NAME).expect("nombre fijo válido");
        let stream = self.connector.connect(name, tcp).await?;
        Secure::new(TlsStream::Client(stream))
    }

    pub async fn accept(&self, tcp: TcpStream) -> Result<Secure> {
        tcp.set_nodelay(true)?;
        let stream = self.acceptor.accept(tcp).await?;
        Secure::new(TlsStream::Server(stream))
    }
}

/// Conexión cifrada junto con la huella comprobada del otro lado.
pub struct Secure {
    pub stream: TlsStream<TcpStream>,
    pub peer: Fingerprint,
}

impl Secure {
    fn new(stream: TlsStream<TcpStream>) -> Result<Self> {
        let peer = stream
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|chain| chain.first())
            .map(|certificate| Fingerprint::of(certificate))
            .ok_or_else(|| Error::Protocol("el otro lado no presentó certificado".to_string()))?;
        Ok(Self { stream, peer })
    }
}
