//! Aegis v0.1 bounded, ordered framing. Cryptography belongs to the transport.
//!
//! A `Framed` instance owns one connection. Any I/O/protocol error, or cancellation
//! during I/O, makes it terminal; discard the connection rather than retrying it.

use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const VERSION: u16 = 1;
pub const MAX_PAYLOAD: usize = 16_384;
pub const MAX_STREAMS: u16 = 16;
pub const HEADER_LEN: usize = 20;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connection I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("payload length {0} exceeds the hard limit")]
    PayloadTooLarge(usize),
    #[error("unsupported frame flags {0:#04x}")]
    Flags(u8),
    #[error("unsupported frame version {0}")]
    Version(u16),
    #[error("unknown frame kind {0}")]
    UnknownKind(u8),
    #[error("unexpected sequence number: expected {expected}, received {actual}")]
    Sequence { expected: u64, actual: u64 },
    #[error("directional sequence counter exhausted")]
    SequenceExhausted,
    #[error("connection framing is terminal after an error or cancellation")]
    Poisoned,
    #[error("invalid message: {0}")]
    InvalidMessage(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Kind {
    Hello = 1,
    Welcome = 2,
    Open = 10,
    OpenOk = 11,
    Data = 12,
    Fin = 13,
    FinAck = 14,
    Ping = 20,
    Pong = 21,
    Close = 30,
    CloseAck = 31,
}

impl TryFrom<u8> for Kind {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::Welcome),
            10 => Ok(Self::Open),
            11 => Ok(Self::OpenOk),
            12 => Ok(Self::Data),
            13 => Ok(Self::Fin),
            14 => Ok(Self::FinAck),
            20 => Ok(Self::Ping),
            21 => Ok(Self::Pong),
            30 => Ok(Self::Close),
            31 => Ok(Self::CloseAck),
            _ => Err(Error::UnknownKind(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub kind: Kind,
    pub stream_id: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(kind: Kind, stream_id: u32, payload: Vec<u8>) -> Self {
        Self {
            kind,
            stream_id,
            payload,
        }
    }
}

/// Parsed header, exposed for allocation-free parser fuzzing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub payload_len: usize,
    pub kind: Kind,
    pub stream_id: u32,
    pub sequence: u64,
}

/// Validate every fixed header field before allocating a payload buffer.
pub fn decode_header(bytes: &[u8]) -> Result<Header> {
    if bytes.len() != HEADER_LEN {
        return Err(Error::InvalidMessage("header must be exactly 20 bytes"));
    }
    let payload_len = u32::from_be_bytes(bytes[0..4].try_into().expect("fixed slice")) as usize;
    if payload_len > MAX_PAYLOAD {
        return Err(Error::PayloadTooLarge(payload_len));
    }
    if bytes[5] != 0 {
        return Err(Error::Flags(bytes[5]));
    }
    let version = u16::from_be_bytes(bytes[6..8].try_into().expect("fixed slice"));
    if version != VERSION {
        return Err(Error::Version(version));
    }
    Ok(Header {
        payload_len,
        kind: Kind::try_from(bytes[4])?,
        stream_id: u32::from_be_bytes(bytes[8..12].try_into().expect("fixed slice")),
        sequence: u64::from_be_bytes(bytes[12..20].try_into().expect("fixed slice")),
    })
}

fn encode_header(frame: &Frame, sequence: u64) -> Result<[u8; HEADER_LEN]> {
    if frame.payload.len() > MAX_PAYLOAD {
        return Err(Error::PayloadTooLarge(frame.payload.len()));
    }
    let mut header = [0_u8; HEADER_LEN];
    header[0..4].copy_from_slice(&(frame.payload.len() as u32).to_be_bytes());
    header[4] = frame.kind as u8;
    header[6..8].copy_from_slice(&VERSION.to_be_bytes());
    header[8..12].copy_from_slice(&frame.stream_id.to_be_bytes());
    header[12..20].copy_from_slice(&sequence.to_be_bytes());
    Ok(header)
}

pub struct Framed<S> {
    io: S,
    send_sequence: Option<u64>,
    recv_sequence: Option<u64>,
    poisoned: bool,
}

impl<S> Framed<S> {
    pub fn new(io: S) -> Self {
        Self {
            io,
            send_sequence: Some(0),
            recv_sequence: Some(0),
            poisoned: false,
        }
    }

    pub fn into_inner(self) -> S {
        self.io
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Framed<S> {
    pub async fn send(&mut self, frame: Frame) -> Result<()> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let sequence = self.send_sequence.ok_or(Error::SequenceExhausted)?;
        let header = encode_header(&frame, sequence)?;
        self.poisoned = true;
        self.io.write_all(&header).await?;
        self.io.write_all(&frame.payload).await?;
        self.io.flush().await?;
        self.send_sequence = sequence.checked_add(1);
        self.poisoned = false;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Frame> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let expected = self.recv_sequence.ok_or(Error::SequenceExhausted)?;
        self.poisoned = true;
        let mut bytes = [0_u8; HEADER_LEN];
        self.io.read_exact(&mut bytes).await?;
        let header = decode_header(&bytes)?;
        if header.sequence != expected {
            return Err(Error::Sequence {
                expected,
                actual: header.sequence,
            });
        }
        let mut payload = vec![0; header.payload_len];
        self.io.read_exact(&mut payload).await?;
        self.recv_sequence = expected.checked_add(1);
        self.poisoned = false;
        Ok(Frame {
            kind: header.kind,
            stream_id: header.stream_id,
            payload,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hello {
    pub min_version: u16,
    pub max_version: u16,
    pub max_payload: u32,
    pub max_streams: u16,
}

impl Default for Hello {
    fn default() -> Self {
        Self {
            min_version: VERSION,
            max_version: VERSION,
            max_payload: MAX_PAYLOAD as u32,
            max_streams: MAX_STREAMS,
        }
    }
}

impl Hello {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&self.min_version.to_be_bytes());
        bytes.extend_from_slice(&self.max_version.to_be_bytes());
        bytes.extend_from_slice(&self.max_payload.to_be_bytes());
        bytes.extend_from_slice(&self.max_streams.to_be_bytes());
        bytes.extend_from_slice(&0_u16.to_be_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 12 {
            return Err(Error::InvalidMessage("Hello must contain exactly 12 bytes"));
        }
        if bytes[10..12] != [0, 0] {
            return Err(Error::InvalidMessage("Hello reserved bits must be zero"));
        }
        Ok(Self {
            min_version: u16::from_be_bytes(bytes[0..2].try_into().expect("fixed slice")),
            max_version: u16::from_be_bytes(bytes[2..4].try_into().expect("fixed slice")),
            max_payload: u32::from_be_bytes(bytes[4..8].try_into().expect("fixed slice")),
            max_streams: u16::from_be_bytes(bytes[8..10].try_into().expect("fixed slice")),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Welcome {
    pub version: u16,
    pub max_streams: u16,
    pub max_payload: u32,
}

impl Welcome {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8);
        bytes.extend_from_slice(&self.version.to_be_bytes());
        bytes.extend_from_slice(&self.max_streams.to_be_bytes());
        bytes.extend_from_slice(&self.max_payload.to_be_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 8 {
            return Err(Error::InvalidMessage(
                "Welcome must contain exactly 8 bytes",
            ));
        }
        Ok(Self {
            version: u16::from_be_bytes(bytes[0..2].try_into().expect("fixed slice")),
            max_streams: u16::from_be_bytes(bytes[2..4].try_into().expect("fixed slice")),
            max_payload: u32::from_be_bytes(bytes[4..8].try_into().expect("fixed slice")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(sequence: u64, payload_len: usize) -> [u8; HEADER_LEN] {
        let frame = Frame::new(Kind::Data, 1, vec![0; payload_len]);
        encode_header(&frame, sequence).unwrap()
    }

    #[tokio::test]
    async fn frame_roundtrips_boundary_payloads_and_both_directions() {
        let (left, right) = tokio::io::duplex(MAX_PAYLOAD + HEADER_LEN);
        let mut sender = Framed::new(left);
        let mut receiver = Framed::new(right);
        for size in [0, 1, 7, 255, 256, 4095, MAX_PAYLOAD - 1, MAX_PAYLOAD] {
            let data: Vec<_> = (0..size).map(|i| (i % 251) as u8).collect();
            let frame = Frame::new(Kind::Data, 1, data);
            sender.send(frame.clone()).await.unwrap();
            assert_eq!(receiver.recv().await.unwrap(), frame);
            receiver.send(frame.clone()).await.unwrap();
            assert_eq!(sender.recv().await.unwrap(), frame);
        }
    }

    #[test]
    fn hello_welcome_roundtrip_and_exact_wire_layout() {
        let hello = Hello::default();
        assert_eq!(Hello::decode(&hello.encode()).unwrap(), hello);
        assert_eq!(hello.encode(), [0, 1, 0, 1, 0, 0, 64, 0, 0, 16, 0, 0]);
        let welcome = Welcome {
            version: VERSION,
            max_streams: MAX_STREAMS,
            max_payload: MAX_PAYLOAD as u32,
        };
        assert_eq!(Welcome::decode(&welcome.encode()).unwrap(), welcome);
        let mut reserved = hello.encode();
        reserved[11] = 1;
        assert!(Hello::decode(&reserved).is_err());
        for len in 0..24 {
            if len != 12 {
                assert!(Hello::decode(&vec![0; len]).is_err());
            }
            if len != 8 {
                assert!(Welcome::decode(&vec![0; len]).is_err());
            }
        }
    }

    #[test]
    fn header_has_canonical_big_endian_layout() {
        let frame = Frame::new(Kind::Data, 0x01020305, vec![9, 8, 7]);
        let bytes = encode_header(&frame, 0x0102030405060708).unwrap();
        assert_eq!(
            bytes,
            [0, 0, 0, 3, 12, 0, 0, 1, 1, 2, 3, 5, 1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(
            decode_header(&bytes).unwrap(),
            Header {
                payload_len: 3,
                kind: Kind::Data,
                stream_id: 0x01020305,
                sequence: 0x0102030405060708
            }
        );
    }

    #[test]
    fn header_rejects_oversized_length_flags_versions_and_unknown_types() {
        let mut bytes = header(0, 0);
        for length in [MAX_PAYLOAD as u32 + 1, u32::MAX] {
            bytes[0..4].copy_from_slice(&length.to_be_bytes());
            assert!(matches!(
                decode_header(&bytes),
                Err(Error::PayloadTooLarge(_))
            ));
        }
        bytes = header(0, 0);
        for flag in 1..=u8::MAX {
            bytes[5] = flag;
            assert!(matches!(decode_header(&bytes), Err(Error::Flags(_))));
        }
        bytes = header(0, 0);
        bytes[6..8].copy_from_slice(&2_u16.to_be_bytes());
        assert!(matches!(decode_header(&bytes), Err(Error::Version(2))));
        bytes = header(0, 0);
        for kind in 0..=u8::MAX {
            bytes[4] = kind;
            assert_eq!(decode_header(&bytes).is_ok(), Kind::try_from(kind).is_ok());
        }
        assert!(decode_header(&[]).is_err());
        assert!(decode_header(&[0; HEADER_LEN - 1]).is_err());
        assert!(decode_header(&[0; HEADER_LEN + 1]).is_err());
    }

    #[tokio::test]
    async fn rejects_replayed_or_out_of_order_sequence_and_poison_connection() {
        for (expected, actual) in [(0, 1), (1, 0), (3, 2), (3, 4)] {
            let (mut writer, reader) = tokio::io::duplex(128);
            let mut framed = Framed::new(reader);
            framed.recv_sequence = Some(expected);
            writer.write_all(&header(actual, 0)).await.unwrap();
            assert!(
                matches!(framed.recv().await, Err(Error::Sequence { expected: e, actual: a }) if e == expected && a == actual)
            );
            assert!(matches!(framed.recv().await, Err(Error::Poisoned)));
        }
    }

    #[tokio::test]
    async fn rejects_oversized_header_without_waiting_for_payload() {
        let (mut writer, reader) = tokio::io::duplex(128);
        let mut bytes = header(0, 0);
        bytes[0..4].copy_from_slice(&u32::MAX.to_be_bytes());
        writer.write_all(&bytes).await.unwrap();
        assert!(matches!(
            Framed::new(reader).recv().await,
            Err(Error::PayloadTooLarge(_))
        ));
    }

    #[tokio::test]
    async fn truncated_header_and_body_fail_closed() {
        for length in 0..HEADER_LEN + 3 {
            let (mut writer, reader) = tokio::io::duplex(128);
            let mut bytes = header(0, 3).to_vec();
            bytes.extend_from_slice(&[1, 2, 3]);
            writer.write_all(&bytes[..length]).await.unwrap();
            writer.shutdown().await.unwrap();
            let mut framed = Framed::new(reader);
            assert!(
                matches!(framed.recv().await, Err(Error::Io(ref e)) if e.kind() == io::ErrorKind::UnexpectedEof)
            );
            assert!(matches!(
                framed.send(Frame::new(Kind::Ping, 0, vec![])).await,
                Err(Error::Poisoned)
            ));
        }
    }

    #[tokio::test]
    async fn sequence_exhaustion_never_wraps() {
        let (left, right) = tokio::io::duplex(128);
        let mut sender = Framed::new(left);
        let mut receiver = Framed::new(right);
        sender.send_sequence = Some(u64::MAX);
        receiver.recv_sequence = Some(u64::MAX);
        sender
            .send(Frame::new(Kind::Ping, 0, vec![0; 8]))
            .await
            .unwrap();
        assert_eq!(receiver.recv().await.unwrap().kind, Kind::Ping);
        assert!(matches!(
            sender.send(Frame::new(Kind::Ping, 0, vec![0; 8])).await,
            Err(Error::SequenceExhausted)
        ));
        assert!(matches!(
            receiver.recv().await,
            Err(Error::SequenceExhausted)
        ));
    }

    #[tokio::test]
    async fn oversized_outgoing_payload_does_not_write_or_consume_sequence() {
        let (left, right) = tokio::io::duplex(128);
        let mut sender = Framed::new(left);
        let mut receiver = Framed::new(right);
        assert!(matches!(
            sender
                .send(Frame::new(Kind::Data, 1, vec![0; MAX_PAYLOAD + 1]))
                .await,
            Err(Error::PayloadTooLarge(_))
        ));
        sender
            .send(Frame::new(Kind::Ping, 0, vec![0; 8]))
            .await
            .unwrap();
        assert_eq!(receiver.recv().await.unwrap().kind, Kind::Ping);
    }

    #[tokio::test]
    async fn cancelling_partial_receive_prevents_reuse() {
        let (mut writer, reader) = tokio::io::duplex(128);
        writer.write_all(&header(0, 3)).await.unwrap();
        writer.write_all(&[1]).await.unwrap();
        let mut receiver = Framed::new(reader);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(5), receiver.recv())
                .await
                .is_err()
        );
        assert!(matches!(receiver.recv().await, Err(Error::Poisoned)));
    }
}
