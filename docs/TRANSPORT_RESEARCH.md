# Transport research plan

Review date: 2026-10-21. No comparative DPI experiment has been executed for this release. Statements below about candidate behavior are design analysis from standards, not measured Aegis results.

## Candidate baseline

| Candidate | Standard basis and motivation | Tradeoff to measure | Status |
| --- | --- | --- | --- |
| Raw TLS 1.3/TCP | RFC 8446; simple authenticated ordered byte stream | Shared TCP head-of-line blocking, nested TCP effects, visible handshake profile/ALPN | Implemented echo transport |
| QUIC/UDP | RFC 9000/9001/9002; independent streams, flow control and path migration | UDP availability, handshake fingerprint, CPU, MTU and migration behavior | Research only |
| HTTP/2 over TLS | RFC 9113; real HTTP framing, CONNECT semantics | TCP head-of-line blocking, proxy behavior, HTTP metadata and stream limits | Research only |
| HTTP/3 over QUIC | RFC 9114; HTTP semantics on QUIC streams | UDP restrictions, HTTP/3 settings/QPACK behavior and intermediary compatibility | Research only |
| CONNECT-UDP | RFC 9298 with HTTP Datagrams/Capsules | Datagram availability, capsule fallback, congestion and proxy policy | Research only |
| CONNECT-IP | RFC 9484; standardized IP tunneling and address/route control | Authentication, policy enforcement, deployment support and inner MTU | Research only |

HTTP CONNECT normally establishes a TCP tunnel. CONNECT-UDP and CONNECT-IP add distinct standardized semantics; they are not interchangeable names for an arbitrary encrypted byte stream. An HTTP transport must actually implement the corresponding HTTP protocol and extension negotiation. Using port 443 or advertising `h2` without implementing HTTP/2 does not make traffic ordinary HTTPS.

The v0.1 ALPN `aegis/0.1` is an explicit classification signal. Standard TLS also leaves observable features such as client/server addresses, connection timing, sizes and parts of the initial handshake. Encrypting application frames prevents reading their content but does not remove those features. No ECH, browser handshake imitation, cover website or probing-resistant front end is implemented.

## Reproducible experiment design

Use owned endpoints and synthetic traffic. Publish server/client commits, dependency locks, OS/kernel versions, CPU, transport configurations, topology, capture filters, clock synchronization and test scripts. Keep secrets and real user traffic out of artifacts.

Compare these controlled conditions with repeated trials:

1. Unrestricted IPv4 and IPv6 paths.
2. UDP dropped completely and UDP selectively rate limited.
3. Induced RTT, jitter, random/burst loss, reordering and bandwidth caps with `tc netem` or an equivalent documented emulator.
4. MTU reduction and blocked ICMP to expose PMTU failure.
5. NAT rebinding/network changes for candidates that claim migration.
6. Explicit transport/protocol filters in a lab; separately label any consented real-network observations with date and provider.

Measure handshake success rate and time, useful-payload throughput, p50/p95 latency, retransmissions/loss, CPU, resident memory, reconnect time and fallback outcome. Distinguish application payload bytes from transport wire bytes. Use a warmup, fixed offered loads, randomized experiment order, sample counts and uncertainty intervals. Store failures as results rather than discarding them.

For fingerprint analysis, compare initial-handshake fields, extension order, negotiated parameters, direction/size sequences, timing distribution, connection duration and response to malformed probes against a documented set of normal applications. A classifier's accuracy on one capture set does not prove recognition resistance on another network. Evaluate false positives on independent held-out traces. Random padding is one variable with bandwidth/latency costs, not the entire strategy.

The transport selected for a later release must satisfy authentication, correctness and resource bounds before availability experiments. A blocked preferred transport may trigger an authenticated fallback; no fallback may accept an unverified certificate or downgrade to plaintext. An adversary able to drop every allowed path can always deny service.

## Primary standards

- [TLS 1.3: RFC 8446](https://www.rfc-editor.org/rfc/rfc8446.html).
- [QUIC transport: RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html), [TLS mapping: RFC 9001](https://www.rfc-editor.org/rfc/rfc9001.html), [loss/congestion: RFC 9002](https://www.rfc-editor.org/rfc/rfc9002.html).
- [HTTP/2: RFC 9113](https://www.rfc-editor.org/rfc/rfc9113.html), [HTTP/3: RFC 9114](https://www.rfc-editor.org/rfc/rfc9114.html).
- [HTTP Datagrams/Capsules: RFC 9297](https://www.rfc-editor.org/rfc/rfc9297.html), [CONNECT-UDP: RFC 9298](https://www.rfc-editor.org/rfc/rfc9298.html), [CONNECT-IP: RFC 9484](https://www.rfc-editor.org/rfc/rfc9484.html).
