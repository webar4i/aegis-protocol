//! Real loopback TCP + TLS tests. Every test generates a fresh disposable CA and
//! private keys; no test credentials are committed or suitable for deployment.
use std::{
    fs, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use aegis_protocol::{Frame, Framed, Hello, Kind, MAX_PAYLOAD, Welcome};
use aegis_transports::{self as transport, rustls};
use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
use tempfile::TempDir;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
    time::timeout,
};

const TEST_TIMEOUT: Duration = Duration::from_secs(15);

type ServerTask = JoinHandle<Result<aegis_core::SessionStats, String>>;
type ClientTls = tokio_rustls::client::TlsStream<tokio::net::TcpStream>;

struct Pki {
    _dir: TempDir,
    ca: PathBuf,
    server_cert: PathBuf,
    server_key: PathBuf,
    client_cert: PathBuf,
    client_key: PathBuf,
    ca_der: rustls::pki_types::CertificateDer<'static>,
}

impl Pki {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];
        let ca_key = KeyPair::generate().unwrap();
        let ca = ca_params.self_signed(&ca_key).unwrap();

        let mut server_params = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
        server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let server_key = KeyPair::generate().unwrap();
        let server = server_params.signed_by(&server_key, &ca, &ca_key).unwrap();

        let mut client_params = CertificateParams::new(vec!["device.test".to_owned()]).unwrap();
        client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let client_key = KeyPair::generate().unwrap();
        let client = client_params.signed_by(&client_key, &ca, &ca_key).unwrap();

        let ca_path = dir.path().join("ca.pem");
        let server_cert = dir.path().join("server.pem");
        let server_key_path = dir.path().join("server-key.pem");
        let client_cert = dir.path().join("client.pem");
        let client_key_path = dir.path().join("client-key.pem");
        fs::write(&ca_path, ca.pem()).unwrap();
        fs::write(&server_cert, server.pem()).unwrap();
        fs::write(&server_key_path, server_key.serialize_pem()).unwrap();
        fs::write(&client_cert, client.pem()).unwrap();
        fs::write(&client_key_path, client_key.serialize_pem()).unwrap();
        Self {
            _dir: dir,
            ca: ca_path,
            server_cert,
            server_key: server_key_path,
            client_cert,
            client_key: client_key_path,
            ca_der: ca.der().clone(),
        }
    }

    fn server_config(&self) -> Arc<rustls::ServerConfig> {
        transport::server_config(&self.ca, &self.server_cert, &self.server_key).unwrap()
    }

    fn client_config(&self) -> Arc<rustls::ClientConfig> {
        transport::client_config(&self.ca, &self.client_cert, &self.client_key).unwrap()
    }
}

async fn spawn_server(config: Arc<rustls::ServerConfig>) -> (String, ServerTask) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    spawn_on_listener(listener, config)
}

fn spawn_on_listener(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
) -> (String, ServerTask) {
    let address = listener.local_addr().unwrap().to_string();
    let task = tokio::spawn(async move {
        timeout(TEST_TIMEOUT, async move {
            let (tcp, _) = listener.accept().await.map_err(|error| error.to_string())?;
            let tls = transport::accept(tcp, config)
                .await
                .map_err(|error| error.to_string())?;
            aegis_core::serve_echo(tls)
                .await
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| "test server timed out".to_owned())?
    });
    (address, task)
}

async fn finish(task: ServerTask) -> Result<aegis_core::SessionStats, String> {
    timeout(TEST_TIMEOUT, task)
        .await
        .expect("server task deadline")
        .expect("server task panicked")
}

async fn secure_client(address: &str, pki: &Pki) -> ClientTls {
    transport::connect(address, "localhost", pki.client_config())
        .await
        .unwrap()
}

async fn recv(wire: &mut Framed<ClientTls>) -> Frame {
    timeout(TEST_TIMEOUT, wire.recv())
        .await
        .expect("receive deadline")
        .unwrap()
}

async fn send(wire: &mut Framed<ClientTls>, kind: Kind, id: u32, payload: Vec<u8>) {
    timeout(TEST_TIMEOUT, wire.send(Frame::new(kind, id, payload)))
        .await
        .expect("send deadline")
        .unwrap();
}

async fn negotiate(wire: &mut Framed<ClientTls>) -> Welcome {
    send(wire, Kind::Hello, 0, Hello::default().encode()).await;
    let frame = recv(wire).await;
    assert_eq!((frame.kind, frame.stream_id), (Kind::Welcome, 0));
    Welcome::decode(&frame.payload).unwrap()
}

async fn assert_rejected(pki: &Pki, config: Arc<rustls::ClientConfig>, server_name: &str) {
    let (address, server) = spawn_server(pki.server_config()).await;
    // TLS1.3 may let the client finish its local handshake before it receives
    // the server's client-auth alert. An application exchange must fail too.
    let accepted = timeout(TEST_TIMEOUT, async {
        match transport::connect(&address, server_name, config).await {
            Ok(tls) => aegis_client::roundtrip(tls, b"must not echo").await.is_ok(),
            Err(_) => false,
        }
    })
    .await
    .expect("rejection deadline");
    assert!(
        !accepted,
        "unauthorized connection exchanged application data"
    );
    assert!(finish(server).await.is_err());
}

#[tokio::test]
async fn tls13_mtls_binary_roundtrips_and_clean_close() {
    let pki = Pki::new();
    for length in [0, MAX_PAYLOAD + 257, aegis_client::MAX_INPUT_BYTES] {
        let (address, server) = spawn_server(pki.server_config()).await;
        let tls = secure_client(&address, &pki).await;
        assert_eq!(
            tls.get_ref().1.protocol_version(),
            Some(rustls::ProtocolVersion::TLSv1_3)
        );
        assert_eq!(tls.get_ref().1.alpn_protocol(), Some(transport::ALPN));
        let input: Vec<u8> = (0..length).map(|i| (i % 251) as u8).collect();
        let output = timeout(TEST_TIMEOUT, aegis_client::roundtrip(tls, &input))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(output, input);
        finish(server).await.unwrap();
    }
}

#[tokio::test]
async fn wrong_server_name_is_rejected() {
    let pki = Pki::new();
    assert_rejected(&pki, pki.client_config(), "other.test").await;
}

#[tokio::test]
async fn untrusted_server_ca_is_rejected() {
    let pki = Pki::new();
    let unrelated = Pki::new();
    let config =
        transport::client_config(&unrelated.ca, &pki.client_cert, &pki.client_key).unwrap();
    assert_rejected(&pki, config, "localhost").await;
}

#[tokio::test]
async fn unauthorized_device_certificate_is_rejected() {
    let pki = Pki::new();
    let unrelated = Pki::new();
    let config =
        transport::client_config(&pki.ca, &unrelated.client_cert, &unrelated.client_key).unwrap();
    assert_rejected(&pki, config, "localhost").await;
}

#[tokio::test]
async fn missing_device_certificate_is_rejected() {
    let pki = Pki::new();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(pki.ca_der.clone()).unwrap();
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.alpn_protocols = vec![transport::ALPN.to_vec()];
    assert_rejected(&pki, Arc::new(config), "localhost").await;
}

#[tokio::test]
async fn missing_or_mismatched_alpn_is_rejected() {
    let pki = Pki::new();
    for protocols in [vec![], vec![b"unrelated/1".to_vec()]] {
        let mut config = pki.client_config();
        Arc::make_mut(&mut config).alpn_protocols = protocols;
        assert_rejected(&pki, config, "localhost").await;
    }
}

#[tokio::test]
async fn independent_streams_preserve_binary_payloads() {
    let pki = Pki::new();
    let (address, server) = spawn_server(pki.server_config()).await;
    let mut wire = Framed::new(secure_client(&address, &pki).await);
    negotiate(&mut wire).await;
    for id in [1, 3] {
        send(&mut wire, Kind::Open, id, vec![]).await;
        assert_eq!(recv(&mut wire).await, Frame::new(Kind::OpenOk, id, vec![]));
    }
    // Queue frames on both logical streams before reading either response.
    send(&mut wire, Kind::Data, 3, vec![0, 255, 7]).await;
    send(&mut wire, Kind::Data, 1, vec![9; MAX_PAYLOAD]).await;
    assert_eq!(
        recv(&mut wire).await,
        Frame::new(Kind::Data, 3, vec![0, 255, 7])
    );
    assert_eq!(
        recv(&mut wire).await,
        Frame::new(Kind::Data, 1, vec![9; MAX_PAYLOAD])
    );
    for id in [3, 1] {
        send(&mut wire, Kind::Fin, id, vec![]).await;
        assert_eq!(recv(&mut wire).await, Frame::new(Kind::FinAck, id, vec![]));
    }
    send(&mut wire, Kind::Close, 0, vec![]).await;
    assert_eq!(recv(&mut wire).await, Frame::new(Kind::CloseAck, 0, vec![]));
    let mut tls = wire.into_inner();
    timeout(TEST_TIMEOUT, tls.shutdown())
        .await
        .unwrap()
        .unwrap();
    let mut trailing = [0u8; 1];
    assert_eq!(
        timeout(TEST_TIMEOUT, tls.read(&mut trailing))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    finish(server).await.unwrap();
}

#[tokio::test]
async fn closed_stream_id_cannot_be_reused() {
    let pki = Pki::new();
    let (address, server) = spawn_server(pki.server_config()).await;
    let mut wire = Framed::new(secure_client(&address, &pki).await);
    negotiate(&mut wire).await;
    send(&mut wire, Kind::Open, 1, vec![]).await;
    recv(&mut wire).await;
    send(&mut wire, Kind::Fin, 1, vec![]).await;
    recv(&mut wire).await;
    send(&mut wire, Kind::Open, 1, vec![]).await;
    assert!(timeout(TEST_TIMEOUT, wire.recv()).await.unwrap().is_err());
    assert!(finish(server).await.is_err());
}

#[tokio::test]
async fn malformed_hello_is_rejected_after_tls() {
    let pki = Pki::new();
    let (address, server) = spawn_server(pki.server_config()).await;
    let mut wire = Framed::new(secure_client(&address, &pki).await);
    send(&mut wire, Kind::Hello, 0, vec![0; 11]).await;
    assert!(timeout(TEST_TIMEOUT, wire.recv()).await.unwrap().is_err());
    assert!(finish(server).await.is_err());
}

/// Forward complete TLS records using their public five-byte record headers.
/// No plaintext or keys are inspected. Once armed, corrupt exactly one
/// ciphertext byte in the next encrypted client record.
async fn forward_with_one_mutation<R, W>(
    mut read: R,
    mut write: W,
    armed: Arc<AtomicBool>,
    mutated: Arc<AtomicBool>,
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    loop {
        let mut header = [0u8; 5];
        match read.read_exact(&mut header).await {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return write.shutdown().await;
            }
            Err(error) => return Err(error),
        }
        // The u16 wire length bounds the relay allocation to at most 65535.
        let length = u16::from_be_bytes([header[3], header[4]]) as usize;
        let mut body = vec![0u8; length];
        read.read_exact(&mut body).await?;
        if header[0] == 23 && armed.swap(false, Ordering::AcqRel) {
            let last = body
                .last_mut()
                .ok_or_else(|| io::Error::other("empty encrypted record"))?;
            *last ^= 1;
            mutated.store(true, Ordering::Release);
        }
        write.write_all(&header).await?;
        write.write_all(&body).await?;
        write.flush().await?;
    }
}

#[tokio::test]
async fn modified_tls_application_record_is_rejected_before_echo() {
    let pki = Pki::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = listener.local_addr().unwrap();
    let config = pki.server_config();
    let server = tokio::spawn(async move {
        timeout(TEST_TIMEOUT, async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let tls = transport::accept(tcp, config).await.unwrap();
            aegis_core::serve_echo(tls).await
        })
        .await
        .expect("mutation server deadline")
    });

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_address = proxy_listener.local_addr().unwrap().to_string();
    let armed = Arc::new(AtomicBool::new(false));
    let mutated = Arc::new(AtomicBool::new(false));
    let relay_armed = armed.clone();
    let relay_mutated = mutated.clone();
    let relay = tokio::spawn(async move {
        timeout(TEST_TIMEOUT, async move {
            let (downstream, _) = proxy_listener.accept().await?;
            let upstream = TcpStream::connect(upstream).await?;
            let (client_read, mut client_write) = downstream.into_split();
            let (mut server_read, server_write) = upstream.into_split();
            let outbound =
                forward_with_one_mutation(client_read, server_write, relay_armed, relay_mutated);
            let inbound = async {
                tokio::io::copy(&mut server_read, &mut client_write).await?;
                client_write.shutdown().await
            };
            tokio::try_join!(outbound, inbound)?;
            Ok::<(), io::Error>(())
        })
        .await
        .expect("mutation relay deadline")
    });

    let mut wire = Framed::new(secure_client(&proxy_address, &pki).await);
    negotiate(&mut wire).await;
    send(&mut wire, Kind::Open, 1, vec![]).await;
    assert_eq!(recv(&mut wire).await, Frame::new(Kind::OpenOk, 1, vec![]));
    // OpenOk proves both the TLS handshake and the initial Aegis exchanges
    // passed intact. The next record belongs to a new DATA application frame.
    armed.store(true, Ordering::Release);
    send(
        &mut wire,
        Kind::Data,
        1,
        b"authenticated payload must never echo after corruption".to_vec(),
    )
    .await;
    assert!(timeout(TEST_TIMEOUT, wire.recv()).await.unwrap().is_err());
    drop(wire);
    assert!(
        mutated.load(Ordering::Acquire),
        "relay did not modify an encrypted record"
    );
    let error = timeout(TEST_TIMEOUT, server)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    match error {
        aegis_core::Error::Protocol(aegis_protocol::Error::Io(error)) => {
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(
                matches!(
                    error
                        .get_ref()
                        .and_then(|source| source.downcast_ref::<rustls::Error>()),
                    Some(rustls::Error::DecryptError)
                ),
                "server must reject the TLS authentication tag, before parsing DATA"
            );
        }
        other => panic!("expected a TLS decryption rejection, received {other}"),
    }
    timeout(TEST_TIMEOUT, relay)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn ipv6_loopback_tls_echo_when_available() {
    let listener = match TcpListener::bind("[::1]:0").await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("SKIP: IPv6 loopback is unavailable in this environment: {error}");
            return;
        }
    };
    let pki = Pki::new();
    let (address, server) = spawn_on_listener(listener, pki.server_config());
    let tls = secure_client(&address, &pki).await;
    let input = b"Aegis over IPv6 TCP loopback";
    let echoed = timeout(TEST_TIMEOUT, aegis_client::roundtrip(tls, input))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(echoed, input);
    finish(server).await.unwrap();
    eprintln!(
        "Verified: IPv6 TCP loopback with TLS1.3 mutual authentication and Aegis echo; no IP tunnel involved"
    );
}

#[tokio::test]
async fn abrupt_tcp_eof_after_close_ack_is_not_clean_tls_shutdown() {
    let pki = Pki::new();
    let (address, server) = spawn_server(pki.server_config()).await;
    let mut wire = Framed::new(secure_client(&address, &pki).await);
    negotiate(&mut wire).await;
    send(&mut wire, Kind::Close, 0, vec![]).await;
    assert_eq!(recv(&mut wire).await, Frame::new(Kind::CloseAck, 0, vec![]));
    // Deliberately omit TLS close_notify. A received protocol acknowledgement
    // alone must not cause the server to count a truncated transport as clean.
    drop(wire);
    assert!(finish(server).await.is_err());
}

#[tokio::test]
async fn application_bytes_after_close_ack_are_rejected() {
    let pki = Pki::new();
    let (address, server) = spawn_server(pki.server_config()).await;
    let mut wire = Framed::new(secure_client(&address, &pki).await);
    negotiate(&mut wire).await;
    send(&mut wire, Kind::Close, 0, vec![]).await;
    assert_eq!(recv(&mut wire).await, Frame::new(Kind::CloseAck, 0, vec![]));
    let mut tls = wire.into_inner();
    // Authenticated bytes after CLOSE are still a state-machine violation.
    timeout(TEST_TIMEOUT, tls.write_all(b"trailing application bytes"))
        .await
        .unwrap()
        .unwrap();
    timeout(TEST_TIMEOUT, tls.flush()).await.unwrap().unwrap();
    let _ = timeout(TEST_TIMEOUT, tls.shutdown()).await.unwrap();
    drop(tls);
    assert!(finish(server).await.is_err());
}
