# Aegis Protocol v0.1

Status: experimental executable specification, wire revision `1`. This revision
is a mutually authenticated, multiplexed **echo transport prototype**. It does
not route IP packets, provide a VPN interface, proxy destinations, or implement
the future control plane. An interoperable Aegis implementation follows the
messages and state transitions below; it does not need Xray-core.

`MUST`, `MUST NOT`, and `SHOULD` express implementation requirements. All
integers are unsigned and use network byte order (big endian). Lengths count
bytes. Every message has exactly one canonical representation.

## 1. Layering and transport

The implementation separates three layers:

1. `aegis-transport`: authenticated, confidential byte stream.
2. `aegis-protocol`: bounded framing, message encoding, directional sequence
   validation.
3. `aegis-core`: version negotiation, session and logical stream state,
   resource limits, and the prototype echo service.

The v0.1 transport is TCP followed by TLS **1.3 only**. TLS 1.2 and cleartext
fallback are prohibited. The ALPN identifier is the ASCII byte string
`aegis/0.1`. Both peers MUST verify that the negotiated protocol is TLS 1.3 and
that the selected ALPN equals this identifier before sending an Aegis frame.

TLS uses `rustls`, its standard TLS 1.3 cipher suites, the `ring` cryptographic
provider, and `tokio-rustls` for asynchronous I/O. Aegis defines no cipher,
signature scheme, key derivation function, nonce construction, or additional
encryption layer. TLS record authentication supplies confidentiality and
integrity for all frame bytes.

Future QUIC, HTTP/2 CONNECT, and HTTP/3 CONNECT transports need separate
transport specifications, capability negotiation, flow control, and
interoperability tests. Advertising HTTP ALPN while sending these raw frames
would not implement HTTP and is prohibited. Current framing requires an
ordered, reliable byte stream; it is not a UDP datagram format.

## 2. Trust, credentials, and TLS handshake

The operator provisions a dedicated CA and separate server and device
certificates outside the wire protocol. Clients receive the trusted CA,
expected server DNS name, client certificate chain, and matching private key.
The server receives its certificate chain/private key and the client trust CA.

The handshake is:

1. The client opens TCP and sends a TLS 1.3 ClientHello with Aegis ALPN.
2. The server presents its certificate chain and requests a client certificate.
3. The client validates the server chain, certificate validity, permitted
   usage, and DNS name/SAN using the configured trust root, then proves
   possession of its client private key.
4. The server validates the client chain, validity, permitted usage, and TLS
   proof of possession against its configured client trust root.
5. Both peers complete TLS and verify the negotiated ALPN and TLS version.
   Only then may Aegis session negotiation begin.

An untrusted or expired chain, incorrect server name, absent client certificate,
invalid signature, wrong ALPN, or wrong TLS version terminates establishment.
There is no `skip certificate validation` option. The CA private key is not
required by either running peer and MUST be kept offline or in a separate
credential issuing service.

**Authorization boundary:** v0.1 authorizes every currently valid client
certificate chaining to the configured client CA. It has no per-device
allowlist, expiry policy beyond certificate validity, CRL/OCSP integration,
revocation API, or certificate issuance endpoint. Use a dedicated CA and
short-lived device certificates for experiments. Do not expose this prototype
as a public commercial service.

TLS session resumption, tickets, TLS 0-RTT/early data, and server half-RTT data
are disabled. Every new connection performs fresh certificate authentication.
Transport establishment has a finite deadline. TLS secrets remain owned by
the TLS implementation and are discarded with that connection; Aegis does not
serialize traffic secrets. Long-lived TLS key-update policy and maximum
production connection lifetime remain future transport work.

## 3. Frame header

Every Aegis frame has a 20-byte header followed immediately by `payload_len`
payload bytes. There is no alignment, padding, compression, checksum, or
optional field in revision 1.

| Offset | Width | Field | Requirement |
| --- | --- | --- | --- |
| 0 | 4 | `payload_len` | `0..=16384`; excludes the header |
| 4 | 1 | `kind` | One of the kinds in section 5 |
| 5 | 1 | `flags` | Exactly zero |
| 6 | 2 | `wire_version` | Exactly `1` |
| 8 | 4 | `stream_id` | Zero for session control; odd nonzero for stream messages |
| 12 | 8 | `sequence` | Strictly increasing counter for this sender |

The receiver MUST read and validate the fixed header before allocating the
payload buffer. It MUST reject unknown kinds, nonzero flags, unsupported wire
versions, and payload sizes above the hard limit. Session code additionally
enforces negotiated payload and stream limits and kind-specific payload
lengths. A frame must be received completely before it can affect session
state or be delivered to the service.

TCP segmentation and TLS record boundaries do not define Aegis frame
boundaries. Several frames may occupy one TLS record, and one frame may span
several records. Implementations MUST handle partial reads and writes.

For example, an empty DATA frame on stream 1 with sender sequence 0 has the
hexadecimal header:

```text
00 00 00 00  0c 00 00 01  00 00 00 01  00 00 00 00 00 00 00 00
```

This example illustrates framing only: DATA before session and stream
establishment is a protocol violation.

## 4. Ordering, replay, and terminal errors

Each direction has an independent counter initialized to zero on a fresh TLS
connection. HELLO and WELCOME consume the first sequence in their respective
directions. A receiver requires the next exact value; duplicate, skipped, or
decreasing sequence numbers terminate the connection. After sequence
`18446744073709551615`, no further frame can be sent in that direction. Counters
MUST NOT wrap.

TLS 1.3 already authenticates record ordering and protects against an observer
replaying encrypted records. Aegis counters additionally validate session
ordering and reject an old Aegis frame carrying its original sequence number.
An authenticated peer can still submit the same DATA payload with a fresh
sequence number. These counters do not create exactly-once delivery across
new connections: an application retry may repeat a request on a newly
authenticated session. The future IP tunnel has no application transaction
deduplication guarantee.

EOF before CLOSE/CLOSE_ACK is an unclean disconnect. A partial header or
payload, transport error, timeout, cancelled frame I/O, parser error, invalid
state transition, or exceeded limit makes the connection terminal. The caller
MUST discard it, close the transport, and MUST NOT retry a partial frame on the
same byte stream. There is no wire ERROR message in revision 1; protocol errors
close the connection and can produce a local technical reason without payload
logging.

## 5. Message definitions

| Kind | Value | Stream | Payload | Direction |
| --- | --- | --- | --- | --- |
| HELLO | 1 | 0 | Exactly 12 bytes, section 6 | Client → server |
| WELCOME | 2 | 0 | Exactly 8 bytes, section 6 | Server → client |
| OPEN | 10 | Odd, nonzero | Empty | Client → server |
| OPEN_OK | 11 | Matching OPEN | Empty | Server → client |
| DATA | 12 | Active stream | `0..=negotiated_max_payload` bytes | Both |
| FIN | 13 | Active stream | Empty | Client → server |
| FIN_ACK | 14 | Matching FIN | Empty | Server → client |
| PING | 20 | 0 | Exactly 8 opaque bytes | Client → server |
| PONG | 21 | 0 | Exact PING payload | Server → client |
| CLOSE | 30 | 0 | Empty | Client → server |
| CLOSE_ACK | 31 | 0 | Empty | Server → client |

The prototype server is an echo endpoint. For each valid DATA received on an
active stream, it emits one DATA on the same stream with exactly the same
payload. This tests protected bidirectional transmission and stream isolation.
It does not accept a destination address or forward traffic to another host.
DATA bytes have no interpreted text encoding and may contain any binary value.

This revision has client-initiated streams and client-initiated graceful
closures only. Server-side idle/limit/shutdown events close the transport;
server-initiated OPEN, FIN, PING, or CLOSE are not defined. Future directions
must be versioned rather than silently accepted.

## 6. Session negotiation

After successful TLS, the client MUST send HELLO as its first Aegis frame.
The server MUST respond with WELCOME as its first Aegis frame before it accepts
OPEN, DATA, PING, or CLOSE.

HELLO payload:

| Offset | Width | Field |
| --- | --- | --- |
| 0 | 2 | Minimum supported session version |
| 2 | 2 | Maximum supported session version |
| 4 | 4 | Requested maximum DATA payload |
| 8 | 2 | Requested maximum simultaneously active streams |
| 10 | 2 | Reserved, exactly zero |

For this implementation, supported session versions are `[1,1]`. The HELLO
range MUST be ordered and contain `1`. Requested limits MUST be nonzero. The
server chooses `min(requested_max_payload, 16384)` and
`min(requested_max_streams, 16)` and may further restrict them through local
policy. No version overlap or invalid fields terminate negotiation.

WELCOME payload:

| Offset | Width | Field |
| --- | --- | --- |
| 0 | 2 | Selected session version, exactly `1` |
| 2 | 2 | Selected active-stream limit |
| 4 | 4 | Selected DATA-payload limit |

The client MUST reject a selected version outside its offered range or its
implemented versions, a zero limit, or a limit exceeding either its offer or
the v0.1 hard cap. The selected limits apply in both directions. Control
payloads have their fixed lengths even if the DATA limit is smaller.

`wire_version` describes the header/envelope grammar, whereas the negotiated
session version describes message semantics. Both equal 1 in this revision.
The ALPN token identifies the bootstrap grammar. A future incompatible
envelope requires a new ALPN token; this HELLO cannot safely negotiate a
different header format. Future compatible session versions require explicit
implementation and tests. An unknown flag or message is fatal rather than
silently ignored.

## 7. Logical streams and teardown

Client stream IDs are odd unsigned 32-bit integers in the range `1..=4294967295`,
strictly increasing for every OPEN. IDs MUST NOT be reused, including after a
FIN acknowledgement, and MUST NOT wrap. An implementation may skip unused
odd IDs. At most the negotiated number may be active simultaneously.

1. Client sends OPEN with a fresh ID and empty payload.
2. Server validates capacity and ID monotonicity, records the active stream,
   then sends OPEN_OK for that ID.
3. Client may send DATA only after OPEN_OK. The echo server returns DATA on
   that same active stream. Bytes on different IDs remain distinct logical
   flows, but all frames share one reliable ordered TLS stream and therefore
   share TCP head-of-line blocking.
4. Client sends FIN after its DATA. Server finishes echoing earlier DATA,
   removes the stream, and sends FIN_ACK. FIN in this revision closes the
   entire logical stream; it is not TCP half-close.
5. After FIN_ACK, neither peer may send DATA for that ID. The ID remains
   permanently consumed for this session.

Invalid IDs, duplicate OPEN, DATA or FIN on an unopened/closed stream, frames
in the wrong direction, unexpected acknowledgements, or any nonempty payload
on an empty-payload message terminate the session. The future IP tunnel will
need its own channel kind and packet semantics; OPEN does not currently mean
a TCP proxy connection or a TUN interface.

The client may send PING during an established session and checks that PONG
contains the identical 8-byte opaque value. No peer wall clock is trusted or
transmitted. PING/PONG measure transport liveness only.

For graceful session shutdown, the client finishes its active streams, sends
CLOSE and waits for CLOSE_ACK. The server requires no active streams and sends
CLOSE_ACK. Both peers then shut down the transport write half and read until a
clean EOF within the transport closure deadline. For TLS this exchanges
`close_notify` in both directions. Any trailing application byte, missing TLS
closure notification, I/O error, or deadline expiry makes the closure unclean.
No later Aegis frame is permitted. TLS `close_notify` remains a transport
operation, separate from these application messages. A graceful exchange does not promise durable
storage of data; this is an in-memory echo service.

## 8. Resource and privacy requirements

All peers enforce the 16 KiB hard payload cap before allocation and negotiated
DATA limits thereafter. The prototype server has the following local defaults:

| Resource | Limit |
| --- | --- |
| Concurrent TLS establishments and sessions combined | 64 |
| TCP/TLS establishment | 10 seconds |
| Aegis HELLO negotiation | 10 seconds |
| Each complete frame read or write | 30 seconds |
| Entire Aegis session, including negotiation | 3,600 seconds |
| Frames per direction, including control frames | 65,536 |
| DATA bytes received plus DATA bytes echoed per session | 16 MiB combined |
| Active logical streams per session | At most 16, negotiated downward |
| Graceful process shutdown after Ctrl-C or Unix SIGTERM | 5 seconds, then abort remaining tasks |

In echo mode, the combined 16 MiB DATA budget permits at most 8 MiB of client
DATA plus its 8 MiB echo. Quota accounting excludes frame headers and TLS/TCP
overhead. An authentication attempt consumes connection capacity until it
finishes or times out. The prototype processes frames serially and uses I/O
backpressure; it does not maintain unbounded per-stream queues. These defaults
are not negotiated as per-user policy or exposed through a management API.

The maximum individual encoded frame is 16,404 bytes. Aegis stream counters do
not limit TLS library buffers, kernel TCP buffers, or all process memory; the
connection cap and deployment controls remain necessary. Quotas, limits, and
deadlines are local implementation policy, not a substitute for a protocol
payload cap. Production per-device policy, fairness, rate limiting, abuse
mitigation, key storage, and metrics need later work.

Implementations MUST NOT log payloads, plaintext traffic contents, private
keys, or TLS traffic secrets. The prototype may report aggregate counts and
local technical failures. Operational deployments must define retention and
the information exposed by connection/address logs.

## 9. Security and censorship properties

The channel protects payload confidentiality and integrity against a network
observer or active intermediary provided certificate validation, CA custody,
endpoint integrity, and the TLS implementation remain sound. A hostile
authorized device can still send invalid frames, consume its allowed
resources, or disconnect. A compromised server can observe plaintext; this
is not end-to-end application encryption beyond that server.

TLS metadata, IP addresses, packet lengths, timing, connection behavior, and
the Aegis ALPN can be observable. This prototype has no browser-fingerprint
imitation, packet-shape obfuscation, active-probing disguise, or measured DPI
resistance. Its dedicated ALPN is an identifiable feature. TLS alone does not
make a protocol unblockable; an operator can block endpoint IPs, transport
types, or suspected traffic. No censorship experiment or throughput claim is
implied by passing localhost functional tests.

See `docs/THREAT_MODEL.md` for the repository threat model and the test report
for actual executed checks. Independent security review and real-network
measurements are required before public deployment.

## 10. Conformance checks and next revision

Required executable checks include canonical encoding, binary DATA roundtrip,
payload boundaries, malformed or truncated headers/bodies, unknown
flags/types/versions, exact sequence enforcement and non-wrapping counters,
state transition rejection, stream limits, and invalid TLS identity/name/ALPN
rejection. Parser fuzz targets complement examples; their existence is not
evidence that a fuzz campaign was run.

The next functional milestone is a Linux TUN adapter and a versioned IP-packet
channel: validate complete IPv4/IPv6 packets and MTU, carry packets without TCP
stream assumptions, configure routes and DNS with rollback, and test outbound
TCP/UDP/IPv4/IPv6 plus DNS and disconnect behavior in isolated network
namespaces. This must precede claims that Aegis v0.1 is a usable VPN.
