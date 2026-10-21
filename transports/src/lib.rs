//! Authenticated byte-stream transports for Aegis.
//!
//! The v0.1 transport is TLS 1.3 over TCP, with mandatory certificate
//! authentication in both directions. It deliberately makes no attempt to
//! imitate a browser or conceal the protocol's ALPN value.

use std::{fs::File, io::Read, path::Path, sync::Arc, time::Duration};

use rustls::{
    ClientConfig, RootCertStore, ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, ServerName, pem::PemObject},
};
use thiserror::Error;
use tokio::{net::TcpStream, time::timeout};
pub use tokio_rustls::rustls;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use zeroize::Zeroizing;

pub const ALPN: &[u8] = b"aegis/0.1";
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Upper bound for each trusted local CA, certificate chain or private key PEM.
pub const MAX_PEM_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TLS configuration error: {0}")]
    Tls(#[from] rustls::Error),
    #[error("TLS certificate verifier configuration failed: {0}")]
    Verifier(#[from] rustls::server::VerifierBuilderError),
    #[error("certificate file contains no certificates")]
    EmptyCertificateChain,
    #[error("key file contains no supported unencrypted private key")]
    MissingPrivateKey,
    // Do not retain the underlying PEM error: IllegalSectionStart can contain
    // the raw input line, which must never appear in Display/Debug diagnostics.
    #[error("invalid PEM data")]
    Pem,
    #[error("TLS credential path must name a regular file")]
    NotRegularFile,
    #[error("TLS credential file exceeds the 1 MiB limit")]
    CredentialFileTooLarge,
    #[error("invalid TLS server name")]
    InvalidServerName,
    #[error("TCP/TLS establishment timed out")]
    HandshakeTimeout,
    #[error("peer did not negotiate the required Aegis ALPN")]
    InvalidAlpn,
    #[error("peer did not negotiate TLS 1.3")]
    InvalidTlsVersion,
    #[error("peer did not provide a certificate")]
    MissingPeerCertificate,
}

pub type Result<T> = std::result::Result<T, TransportError>;

fn pem_bytes(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    // These are administrator-controlled local configuration paths. Check
    // before opening, so an accidentally supplied FIFO cannot block startup.
    // Recheck the opened handle, and independently bound reads if the regular
    // file grows after metadata was read. This is not a sandbox for paths an
    // adversary can concurrently replace.
    if !std::fs::metadata(path)?.is_file() {
        return Err(TransportError::NotRegularFile);
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(TransportError::NotRegularFile);
    }
    if metadata.len() > MAX_PEM_BYTES as u64 {
        return Err(TransportError::CredentialFileTooLarge);
    }
    let mut input = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    file.take((MAX_PEM_BYTES + 1) as u64)
        .read_to_end(&mut input)?;
    if input.len() > MAX_PEM_BYTES {
        return Err(TransportError::CredentialFileTooLarge);
    }
    Ok(input)
}

fn certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let input = pem_bytes(path)?;
    let certs = CertificateDer::pem_slice_iter(&input)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| TransportError::Pem)?;
    if certs.is_empty() {
        return Err(TransportError::EmptyCertificateChain);
    }
    Ok(certs)
}

fn private_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let input = pem_bytes(path)?;
    PrivateKeyDer::from_pem_slice(&input).map_err(|error| match error {
        rustls::pki_types::pem::Error::NoItemsFound => TransportError::MissingPrivateKey,
        _ => TransportError::Pem,
    })
}

fn roots(path: &Path) -> Result<RootCertStore> {
    let mut roots = RootCertStore::empty();
    for certificate in certificates(path)? {
        // Fail closed: a malformed root must not be silently skipped.
        roots.add(certificate)?;
    }
    Ok(roots)
}

/// Load the trusted client CA(s) and the server's certificate/private key.
///
/// Every certificate chaining to the client CA is authorized in v0.1; use a
/// dedicated CA and issue a separate certificate per device. Live revocation,
/// user accounts and quotas are outside this prototype.
pub fn server_config(ca: &Path, cert: &Path, key: &Path) -> Result<Arc<ServerConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots(ca)?),
        provider.clone(),
    )
    .build()?;
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(verifier)
        .with_single_cert(certificates(cert)?, private_key(key)?)?;
    config.alpn_protocols = vec![ALPN.to_vec()];
    config.max_early_data_size = 0;
    config.send_half_rtt_data = false;
    config.send_tls13_tickets = 0;
    config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    Ok(Arc::new(config))
}

/// Load the trusted server CA(s) and the device's certificate/private key.
/// Server hostname and certificate chain validation remain mandatory.
pub fn client_config(ca: &Path, cert: &Path, key: &Path) -> Result<Arc<ClientConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_root_certificates(roots(ca)?)
        .with_client_auth_cert(certificates(cert)?, private_key(key)?)?;
    config.alpn_protocols = vec![ALPN.to_vec()];
    config.enable_early_data = false;
    config.resumption = rustls::client::Resumption::disabled();
    Ok(Arc::new(config))
}

fn check_negotiation(
    alpn: Option<&[u8]>,
    version: Option<rustls::ProtocolVersion>,
    peer_certificates: Option<&[CertificateDer<'_>]>,
) -> Result<()> {
    if alpn != Some(ALPN) {
        return Err(TransportError::InvalidAlpn);
    }
    if version != Some(rustls::ProtocolVersion::TLSv1_3) {
        return Err(TransportError::InvalidTlsVersion);
    }
    if peer_certificates.is_none_or(|certs| certs.is_empty()) {
        return Err(TransportError::MissingPeerCertificate);
    }
    Ok(())
}

/// Open TCP and complete mutual TLS within a single bounded deadline.
/// `server_name` must match the certificate's SAN; it is separate from `addr`
/// so an explicit IP address can be used without weakening name verification.
pub async fn connect(
    addr: &str,
    server_name: &str,
    config: Arc<ClientConfig>,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let name = ServerName::try_from(server_name.to_owned())
        .map_err(|_| TransportError::InvalidServerName)?;
    let tls = timeout(HANDSHAKE_TIMEOUT, async {
        let tcp = TcpStream::connect(addr).await?;
        tcp.set_nodelay(true)?;
        TlsConnector::from(config).connect(name, tcp).await
    })
    .await
    .map_err(|_| TransportError::HandshakeTimeout)??;
    let (_, connection) = tls.get_ref();
    check_negotiation(
        connection.alpn_protocol(),
        connection.protocol_version(),
        connection.peer_certificates(),
    )?;
    Ok(tls)
}

/// Complete mandatory client certificate authentication on an accepted TCP
/// stream. Callers must acquire a global connection permit *before* spawning
/// this operation, to bound unauthenticated handshake work.
pub async fn accept(
    tcp: TcpStream,
    config: Arc<ServerConfig>,
) -> Result<tokio_rustls::server::TlsStream<TcpStream>> {
    tcp.set_nodelay(true)?;
    let tls = timeout(HANDSHAKE_TIMEOUT, TlsAcceptor::from(config).accept(tcp))
        .await
        .map_err(|_| TransportError::HandshakeTimeout)??;
    let (_, connection) = tls.get_ref();
    check_negotiation(
        connection.alpn_protocol(),
        connection.protocol_version(),
        connection.peer_certificates(),
    )?;
    Ok(tls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_certificate_and_key_files() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(matches!(
            certificates(file.path()),
            Err(TransportError::EmptyCertificateChain)
        ));
        assert!(matches!(
            private_key(file.path()),
            Err(TransportError::MissingPrivateKey)
        ));
        assert!(server_config(file.path(), file.path(), file.path()).is_err());
        assert!(client_config(file.path(), file.path(), file.path()).is_err());
    }

    #[test]
    fn rejects_large_or_nonregular_credential_files() {
        let directory = tempfile::tempdir().unwrap();
        assert!(matches!(
            pem_bytes(directory.path()),
            Err(TransportError::NotRegularFile)
        ));
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len((MAX_PEM_BYTES + 1) as u64).unwrap();
        assert!(matches!(
            pem_bytes(file.path()),
            Err(TransportError::CredentialFileTooLarge)
        ));
    }

    #[test]
    fn malformed_pem_diagnostics_do_not_expose_input() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let secret = "sensitive-private-key-material";
        std::fs::write(file.path(), format!("-----BEGIN {secret}\n")).unwrap();
        let error = private_key(file.path()).unwrap_err();
        assert!(matches!(error, TransportError::Pem));
        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
    }

    #[test]
    fn negotiation_checks_fail_closed() {
        assert!(matches!(
            check_negotiation(None, Some(rustls::ProtocolVersion::TLSv1_3), None),
            Err(TransportError::InvalidAlpn)
        ));
        assert!(matches!(
            check_negotiation(Some(ALPN), Some(rustls::ProtocolVersion::TLSv1_2), None),
            Err(TransportError::InvalidTlsVersion)
        ));
        assert!(matches!(
            check_negotiation(Some(ALPN), Some(rustls::ProtocolVersion::TLSv1_3), None),
            Err(TransportError::MissingPeerCertificate)
        ));
    }
}
