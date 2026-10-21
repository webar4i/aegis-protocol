//! Transport-independent Aegis v0.1 session state machine.
//!
//! The only application in this prototype is bounded, multiplexed binary echo.
//! Nothing is routed to the Internet and this crate does not create a TUN device.

use std::{collections::BTreeSet, time::Duration};

use aegis_protocol::{Frame, Framed, Hello, Kind, MAX_PAYLOAD, MAX_STREAMS, VERSION, Welcome};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;

/// A session may transfer at most 16 MiB of DATA in both directions combined.
pub const MAX_SESSION_DATA_BYTES: u64 = 16 * 1024 * 1024;
/// Each direction is limited to this many frames, including control frames.
pub const MAX_SESSION_FRAMES: u64 = 65_536;
pub const NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(10);
pub const FRAME_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_SESSION_DURATION: Duration = Duration::from_secs(3_600);

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid protocol frame: {0}")]
    Protocol(#[from] aegis_protocol::Error),
    #[error("invalid session state: {0}")]
    State(&'static str),
    #[error("session resource limit exceeded: {0}")]
    ResourceLimit(&'static str),
    #[error("session timeout: {0}")]
    Timeout(&'static str),
    #[error("transport closure failed")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Technical counters only. These do not contain destinations or payload data.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SessionStats {
    pub received_frames: u64,
    pub sent_frames: u64,
    pub received_data_bytes: u64,
    pub sent_data_bytes: u64,
    pub streams_opened: u64,
}

/// Serve a strictly negotiated v0.1 echo session on an authenticated transport.
///
/// Callers MUST authenticate the peer before invoking this function (the server
/// binary uses TLS 1.3 mutual authentication). Frame deadlines cover the whole
/// frame read/write, so partial-byte trickles do not reset the deadline.
pub async fn serve_echo<S>(io: S) -> Result<SessionStats>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    timeout(MAX_SESSION_DURATION, serve_echo_inner(io))
        .await
        .map_err(|_| Error::Timeout("session lifetime"))?
}

async fn receive<S>(
    framed: &mut Framed<S>,
    stats: &mut SessionStats,
    wait: Duration,
) -> Result<Frame>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if stats.received_frames >= MAX_SESSION_FRAMES {
        return Err(Error::ResourceLimit("received frame count"));
    }
    let frame = timeout(wait, framed.recv())
        .await
        .map_err(|_| Error::Timeout("frame read"))??;
    stats.received_frames += 1;
    Ok(frame)
}

async fn send<S>(framed: &mut Framed<S>, stats: &mut SessionStats, frame: Frame) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if stats.sent_frames >= MAX_SESSION_FRAMES {
        return Err(Error::ResourceLimit("sent frame count"));
    }
    timeout(FRAME_TIMEOUT, framed.send(frame))
        .await
        .map_err(|_| Error::Timeout("frame write"))??;
    stats.sent_frames += 1;
    Ok(())
}

fn control(frame: &Frame, length: usize) -> Result<()> {
    if frame.stream_id != 0 || frame.payload.len() != length {
        return Err(Error::State("malformed control message"));
    }
    Ok(())
}

fn stream(frame: &Frame, empty_payload: bool) -> Result<()> {
    if frame.stream_id == 0 || frame.stream_id.is_multiple_of(2) {
        return Err(Error::State("stream identifiers must be nonzero and odd"));
    }
    if empty_payload && !frame.payload.is_empty() {
        return Err(Error::State("stream control message must be empty"));
    }
    Ok(())
}

async fn close_transport<S>(mut io: S) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    timeout(FRAME_TIMEOUT, io.shutdown())
        .await
        .map_err(|_| Error::Timeout("transport shutdown"))??;
    // A write-half shutdown flushes our TLS close_notify but does not consume
    // the peer's notification. Retain the read half until a clean EOF to avoid
    // dropping a TCP socket while the peer's closure bytes remain unread.
    let mut trailing = [0_u8; 1];
    let received = timeout(FRAME_TIMEOUT, io.read(&mut trailing))
        .await
        .map_err(|_| Error::Timeout("peer transport closure"))??;
    if received != 0 {
        return Err(Error::State("application data after session close"));
    }
    Ok(())
}

async fn serve_echo_inner<S>(io: S) -> Result<SessionStats>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut framed = Framed::new(io);
    let mut stats = SessionStats::default();
    let hello_frame = receive(&mut framed, &mut stats, NEGOTIATION_TIMEOUT).await?;
    if hello_frame.kind != Kind::Hello {
        return Err(Error::State("first frame must be Hello"));
    }
    control(&hello_frame, 12)?;
    let hello = Hello::decode(&hello_frame.payload)?;
    if hello.min_version > VERSION || hello.max_version < VERSION {
        return Err(Error::State("no supported protocol version"));
    }
    if hello.min_version > hello.max_version || hello.max_payload == 0 || hello.max_streams == 0 {
        return Err(Error::State("invalid negotiation limits"));
    }
    let max_payload = hello.max_payload.min(MAX_PAYLOAD as u32) as usize;
    let max_streams = hello.max_streams.min(MAX_STREAMS);
    let welcome = Welcome {
        version: VERSION,
        max_streams,
        max_payload: max_payload as u32,
    };
    send(
        &mut framed,
        &mut stats,
        Frame::new(Kind::Welcome, 0, welcome.encode()),
    )
    .await?;

    let mut streams = BTreeSet::new();
    let mut last_stream_id = 0;
    loop {
        let frame = receive(&mut framed, &mut stats, FRAME_TIMEOUT).await?;
        match frame.kind {
            Kind::Open => {
                stream(&frame, true)?;
                if frame.stream_id <= last_stream_id {
                    return Err(Error::State("stream identifier reused or not increasing"));
                }
                if streams.len() >= usize::from(max_streams) {
                    return Err(Error::ResourceLimit("concurrent stream count"));
                }
                last_stream_id = frame.stream_id;
                streams.insert(frame.stream_id);
                stats.streams_opened += 1;
                send(
                    &mut framed,
                    &mut stats,
                    Frame::new(Kind::OpenOk, frame.stream_id, Vec::new()),
                )
                .await?;
            }
            Kind::Data => {
                stream(&frame, false)?;
                if !streams.contains(&frame.stream_id) {
                    return Err(Error::State("DATA on an unopened stream"));
                }
                if frame.payload.len() > max_payload {
                    return Err(Error::ResourceLimit("negotiated DATA payload size"));
                }
                let len = frame.payload.len() as u64;
                let aggregate = stats.received_data_bytes + stats.sent_data_bytes;
                if len * 2 > MAX_SESSION_DATA_BYTES - aggregate {
                    return Err(Error::ResourceLimit("bidirectional DATA byte budget"));
                }
                stats.received_data_bytes += len;
                send(&mut framed, &mut stats, frame).await?;
                stats.sent_data_bytes += len;
            }
            Kind::Fin => {
                stream(&frame, true)?;
                if !streams.remove(&frame.stream_id) {
                    return Err(Error::State("FIN on an unopened stream"));
                }
                send(
                    &mut framed,
                    &mut stats,
                    Frame::new(Kind::FinAck, frame.stream_id, Vec::new()),
                )
                .await?;
            }
            Kind::Ping => {
                control(&frame, 8)?;
                send(
                    &mut framed,
                    &mut stats,
                    Frame::new(Kind::Pong, 0, frame.payload),
                )
                .await?;
            }
            Kind::Close => {
                control(&frame, 0)?;
                if !streams.is_empty() {
                    return Err(Error::State("close requires all streams to finish"));
                }
                send(
                    &mut framed,
                    &mut stats,
                    Frame::new(Kind::CloseAck, 0, Vec::new()),
                )
                .await?;
                close_transport(framed.into_inner()).await?;
                return Ok(stats);
            }
            _ => return Err(Error::State("unexpected message in established session")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{DuplexStream, duplex};

    fn hello(payload: u32, streams: u16) -> Frame {
        Frame::new(
            Kind::Hello,
            0,
            Hello {
                min_version: VERSION,
                max_version: VERSION,
                max_payload: payload,
                max_streams: streams,
            }
            .encode(),
        )
    }

    async fn negotiated(
        payload: u32,
        streams: u16,
    ) -> (
        Framed<DuplexStream>,
        tokio::task::JoinHandle<Result<SessionStats>>,
    ) {
        let (client, server) = duplex(64 * 1024);
        let task = tokio::spawn(serve_echo(server));
        let mut client = Framed::new(client);
        client.send(hello(payload, streams)).await.unwrap();
        assert_eq!(client.recv().await.unwrap().kind, Kind::Welcome);
        (client, task)
    }

    async fn open(client: &mut Framed<DuplexStream>, id: u32) {
        client
            .send(Frame::new(Kind::Open, id, Vec::new()))
            .await
            .unwrap();
        let frame = client.recv().await.unwrap();
        assert_eq!(frame.kind, Kind::OpenOk);
        assert_eq!(frame.stream_id, id);
    }

    #[tokio::test]
    async fn multiplex_echo_ping_and_graceful_close() {
        let (mut client, task) = negotiated(1_024, 2).await;
        open(&mut client, 1).await;
        open(&mut client, 3).await;
        for (id, data) in [(3, b"\0binary\xff".as_slice()), (1, b"hello".as_slice())] {
            client
                .send(Frame::new(Kind::Data, id, data.to_vec()))
                .await
                .unwrap();
            let echo = client.recv().await.unwrap();
            assert_eq!(echo.kind, Kind::Data);
            assert_eq!(echo.stream_id, id);
            assert_eq!(echo.payload, data);
        }
        client
            .send(Frame::new(Kind::Ping, 0, 42_u64.to_be_bytes().to_vec()))
            .await
            .unwrap();
        let pong = client.recv().await.unwrap();
        assert_eq!(pong.kind, Kind::Pong);
        assert_eq!(pong.payload, 42_u64.to_be_bytes());
        for id in [1, 3] {
            client
                .send(Frame::new(Kind::Fin, id, Vec::new()))
                .await
                .unwrap();
            assert_eq!(client.recv().await.unwrap().kind, Kind::FinAck);
        }
        client
            .send(Frame::new(Kind::Close, 0, Vec::new()))
            .await
            .unwrap();
        assert_eq!(client.recv().await.unwrap().kind, Kind::CloseAck);
        drop(client);
        let stats = task.await.unwrap().unwrap();
        assert_eq!(stats.streams_opened, 2);
        assert_eq!(stats.received_data_bytes, 13);
        assert_eq!(stats.sent_data_bytes, 13);
    }

    #[tokio::test]
    async fn data_before_hello_is_rejected() {
        let (client, server) = duplex(1024);
        let task = tokio::spawn(serve_echo(server));
        Framed::new(client)
            .send(Frame::new(Kind::Data, 1, vec![1]))
            .await
            .unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::State(_))));
    }

    #[tokio::test]
    async fn unopened_stream_is_rejected() {
        let (mut client, task) = negotiated(1024, 2).await;
        client
            .send(Frame::new(Kind::Data, 1, vec![1]))
            .await
            .unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::State(_))));
    }

    #[tokio::test]
    async fn negotiated_stream_limit_is_enforced() {
        let (mut client, task) = negotiated(1024, 1).await;
        open(&mut client, 1).await;
        client
            .send(Frame::new(Kind::Open, 3, Vec::new()))
            .await
            .unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::ResourceLimit(_))));
    }

    #[tokio::test]
    async fn negotiated_payload_limit_is_enforced() {
        let (mut client, task) = negotiated(1, 1).await;
        open(&mut client, 1).await;
        client
            .send(Frame::new(Kind::Data, 1, vec![1, 2]))
            .await
            .unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::ResourceLimit(_))));
    }

    #[tokio::test]
    async fn finished_stream_cannot_be_reused() {
        let (mut client, task) = negotiated(1024, 1).await;
        open(&mut client, 1).await;
        client
            .send(Frame::new(Kind::Fin, 1, Vec::new()))
            .await
            .unwrap();
        assert_eq!(client.recv().await.unwrap().kind, Kind::FinAck);
        client
            .send(Frame::new(Kind::Open, 1, Vec::new()))
            .await
            .unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::State(_))));
    }

    #[tokio::test]
    async fn close_with_active_stream_is_rejected() {
        let (mut client, task) = negotiated(1024, 1).await;
        open(&mut client, 1).await;
        client
            .send(Frame::new(Kind::Close, 0, Vec::new()))
            .await
            .unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::State(_))));
    }

    #[tokio::test]
    async fn zero_negotiated_limits_are_rejected() {
        for (payload, streams) in [(0, 1), (1, 0)] {
            let (client, server) = duplex(1024);
            let task = tokio::spawn(serve_echo(server));
            Framed::new(client)
                .send(hello(payload, streams))
                .await
                .unwrap();
            assert!(matches!(task.await.unwrap(), Err(Error::State(_))));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn partial_hello_does_not_reset_negotiation_deadline() {
        let (mut client, server) = duplex(1024);
        let task = tokio::spawn(serve_echo(server));
        // Send only a single header byte and keep the writer alive. A complete
        // frame deadline must still expire, even though the connection is open.
        client.write_all(&[0]).await.unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::Timeout(_))));
    }

    #[tokio::test]
    async fn aggregate_data_budget_is_enforced() {
        let (mut client, task) = negotiated(MAX_PAYLOAD as u32, 1).await;
        open(&mut client, 1).await;
        let payload = vec![0xa5; MAX_PAYLOAD];
        for _ in 0..MAX_SESSION_DATA_BYTES / (2 * MAX_PAYLOAD as u64) {
            client
                .send(Frame::new(Kind::Data, 1, payload.clone()))
                .await
                .unwrap();
            assert_eq!(client.recv().await.unwrap().payload, payload);
        }
        client
            .send(Frame::new(Kind::Data, 1, vec![0]))
            .await
            .unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::ResourceLimit(_))));
    }

    #[tokio::test]
    async fn trailing_application_data_prevents_clean_close() {
        let (mut client, server) = duplex(64);
        client.write_all(&[1]).await.unwrap();
        assert!(matches!(
            close_transport(server).await,
            Err(Error::State(_))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn peer_closure_has_a_finite_deadline() {
        let (_client, server) = duplex(64);
        assert!(matches!(
            close_transport(server).await,
            Err(Error::Timeout(_))
        ));
    }
}
