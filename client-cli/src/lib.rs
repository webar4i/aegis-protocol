//! Strict client for the bounded Aegis v0.1 echo prototype.
//!
//! This crate exchanges application frames over an already authenticated secure
//! stream. Production callers must use the TLS adapter from `aegis-transports`.

use aegis_protocol::{Frame, Framed, Hello, Kind, MAX_PAYLOAD, MAX_STREAMS, VERSION, Welcome};
use std::{future::Future, time::Duration};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Bounds input and the retained verification buffer for the demonstration CLI.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("input exceeds the 1 MiB prototype limit")]
    InputTooLarge,
    #[error("session operation timed out")]
    Timeout,
    #[error("protocol operation failed: {0}")]
    Protocol(String),
    #[error("peer sent an unexpected frame")]
    UnexpectedFrame,
    #[error("peer selected invalid session parameters")]
    InvalidNegotiation,
    #[error("echo integrity check failed")]
    Integrity,
    #[error("peer sent application data after session close")]
    TrailingData,
}

async fn bounded<T, E: std::fmt::Display>(
    operation: impl Future<Output = Result<T, E>>,
) -> Result<T, ClientError> {
    tokio::time::timeout(IO_TIMEOUT, operation)
        .await
        .map_err(|_| ClientError::Timeout)?
        .map_err(|error| ClientError::Protocol(error.to_string()))
}

async fn expect<S: AsyncRead + AsyncWrite + Unpin>(
    wire: &mut Framed<S>,
    kind: Kind,
    stream_id: u32,
) -> Result<Frame, ClientError> {
    let frame = bounded(wire.recv()).await?;
    if frame.kind != kind || frame.stream_id != stream_id {
        return Err(ClientError::UnexpectedFrame);
    }
    Ok(frame)
}

async fn expect_empty<S: AsyncRead + AsyncWrite + Unpin>(
    wire: &mut Framed<S>,
    kind: Kind,
    stream_id: u32,
) -> Result<(), ClientError> {
    let frame = expect(wire, kind, stream_id).await?;
    if !frame.payload.is_empty() {
        return Err(ClientError::UnexpectedFrame);
    }
    Ok(())
}

async fn close_transport<S: AsyncRead + AsyncWrite + Unpin>(mut io: S) -> Result<(), ClientError> {
    bounded(io.shutdown()).await?;
    let mut trailing = [0_u8; 1];
    if bounded(io.read(&mut trailing)).await? != 0 {
        return Err(ClientError::TrailingData);
    }
    Ok(())
}

/// Negotiates a session, checks a byte-for-byte echo, and closes it cleanly.
///
/// Frame sequence numbers and wire bounds are enforced by `Framed`; this client
/// additionally enforces message order, stream IDs, and negotiated bounds. Every
/// read and write has a timeout. This function does not create an IP tunnel.
pub async fn roundtrip<S: AsyncRead + AsyncWrite + Unpin>(
    io: S,
    input: &[u8],
) -> Result<Vec<u8>, ClientError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(ClientError::InputTooLarge);
    }

    let mut wire = Framed::new(io);
    let hello = Hello {
        min_version: VERSION,
        max_version: VERSION,
        max_payload: MAX_PAYLOAD as u32,
        max_streams: MAX_STREAMS,
    };
    bounded(wire.send(Frame::new(Kind::Hello, 0, hello.encode()))).await?;
    let welcome_frame = expect(&mut wire, Kind::Welcome, 0).await?;
    let welcome = Welcome::decode(&welcome_frame.payload)
        .map_err(|error| ClientError::Protocol(error.to_string()))?;
    if welcome.version != VERSION
        || welcome.max_payload == 0
        || welcome.max_payload > hello.max_payload
        || welcome.max_streams == 0
        || welcome.max_streams > hello.max_streams
    {
        return Err(ClientError::InvalidNegotiation);
    }

    bounded(wire.send(Frame::new(Kind::Open, 1, Vec::new()))).await?;
    expect_empty(&mut wire, Kind::OpenOk, 1).await?;

    let mut output = Vec::with_capacity(input.len());
    // Exercise the empty-data case too: it is a valid Data message, not EOF.
    if input.is_empty() {
        bounded(wire.send(Frame::new(Kind::Data, 1, Vec::new()))).await?;
        let echoed = expect(&mut wire, Kind::Data, 1).await?;
        if !echoed.payload.is_empty() {
            return Err(ClientError::Integrity);
        }
    } else {
        for chunk in input.chunks(welcome.max_payload as usize) {
            bounded(wire.send(Frame::new(Kind::Data, 1, chunk.to_vec()))).await?;
            let echoed = expect(&mut wire, Kind::Data, 1).await?;
            if echoed.payload != chunk {
                return Err(ClientError::Integrity);
            }
            output.extend_from_slice(&echoed.payload);
        }
    }

    bounded(wire.send(Frame::new(Kind::Fin, 1, Vec::new()))).await?;
    expect_empty(&mut wire, Kind::FinAck, 1).await?;

    // A fixed diagnostic value is not a secret or an authentication nonce.
    let ping = b"aegis001".to_vec();
    bounded(wire.send(Frame::new(Kind::Ping, 0, ping.clone()))).await?;
    if expect(&mut wire, Kind::Pong, 0).await?.payload != ping {
        return Err(ClientError::UnexpectedFrame);
    }

    bounded(wire.send(Frame::new(Kind::Close, 0, Vec::new()))).await?;
    expect_empty(&mut wire, Kind::CloseAck, 0).await?;
    // Flush our close_notify and consume the peer's closure before dropping
    // TCP. CLOSE_ACK alone is not a clean bidirectional TLS shutdown.
    close_transport(wire.into_inner()).await?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn trailing_application_data_prevents_clean_close() {
        let (mut peer, client) = tokio::io::duplex(64);
        peer.write_all(&[1]).await.unwrap();
        assert!(matches!(
            close_transport(client).await,
            Err(ClientError::TrailingData)
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn peer_closure_has_a_finite_deadline() {
        let (_peer, client) = tokio::io::duplex(64);
        assert!(matches!(
            close_transport(client).await,
            Err(ClientError::Timeout)
        ));
    }

    #[tokio::test]
    async fn rejects_oversized_input_before_reading_or_writing() {
        let (io, _other) = tokio::io::duplex(1);
        let input = vec![0_u8; MAX_INPUT_BYTES + 1];
        assert!(matches!(
            roundtrip(io, &input).await,
            Err(ClientError::InputTooLarge)
        ));
    }

    #[tokio::test]
    async fn rejects_unexpected_first_frame() {
        let (client, server) = tokio::io::duplex(1024);
        let peer = tokio::spawn(async move {
            let mut wire = Framed::new(server);
            wire.recv().await.unwrap();
            wire.send(Frame::new(Kind::OpenOk, 1, Vec::new()))
                .await
                .unwrap();
        });
        assert!(matches!(
            roundtrip(client, b"test").await,
            Err(ClientError::UnexpectedFrame)
        ));
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_excessive_negotiated_payload() {
        let (client, server) = tokio::io::duplex(1024);
        let peer = tokio::spawn(async move {
            let mut wire = Framed::new(server);
            wire.recv().await.unwrap();
            let welcome = Welcome {
                version: VERSION,
                max_streams: 1,
                max_payload: MAX_PAYLOAD as u32 + 1,
            };
            wire.send(Frame::new(Kind::Welcome, 0, welcome.encode()))
                .await
                .unwrap();
        });
        assert!(matches!(
            roundtrip(client, b"test").await,
            Err(ClientError::InvalidNegotiation)
        ));
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_corrupted_application_data() {
        let (client, server) = tokio::io::duplex(1024);
        let peer = tokio::spawn(async move {
            let mut wire = Framed::new(server);
            wire.recv().await.unwrap();
            let welcome = Welcome {
                version: VERSION,
                max_streams: 1,
                max_payload: MAX_PAYLOAD as u32,
            };
            wire.send(Frame::new(Kind::Welcome, 0, welcome.encode()))
                .await
                .unwrap();
            wire.recv().await.unwrap();
            wire.send(Frame::new(Kind::OpenOk, 1, Vec::new()))
                .await
                .unwrap();
            wire.recv().await.unwrap();
            wire.send(Frame::new(Kind::Data, 1, b"wrong".to_vec()))
                .await
                .unwrap();
        });
        assert!(matches!(
            roundtrip(client, b"test").await,
            Err(ClientError::Integrity)
        ));
        peer.await.unwrap();
    }
}
