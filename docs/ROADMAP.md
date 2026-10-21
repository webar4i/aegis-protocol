# Roadmap and acceptance criteria

Dates are deliberately omitted: each stage depends on the previous acceptance gate. The initial release is a protocol/echo prototype, not a system VPN.

| Stage | Deliverable | Acceptance gate |
| --- | --- | --- |
| 1–2: v0.1 | Specification, mTLS transport, Rust echo server/CLI, parser and integration tests | Reproducible round trip; wrong identity and malformed sessions fail; exact checks reported in [docs/VALIDATION.md](VALIDATION.md) |
| 3: Linux IP tunnel | Versioned IP capability, TUN adapters, controlled routing | TCP/UDP, IPv4/IPv6, DNS, MTU, reconnect and leak tests in isolated network namespaces |
| 4: Android/Windows | Flutter UI and Rust FFI; native VPN/service adapters | Installable builds, lifecycle/permission tests, route/DNS rollback, kill-switch tests where implemented |
| 5: iOS/macOS/Linux GUI | Native packet-tunnel adapters and platform packaging | Device/native-architecture tests, entitlement/signing verification, platform capability matrix |
| 6: Service management | Go REST API/PostgreSQL, device authorization, node policy, 3x-ui adapter | Credential revoke/expiry reaches nodes; idempotent accounting; privileges separated; integration contract tests |
| 7: Transport research | QUIC and standards-compliant HTTP candidates; reproducible comparison | Published methodology, environment, raw synthetic measurements and uncertainty; no unsupported anti-DPI claim |

## Immediate next task: isolated Linux TUN round trip

Specify a new IP-packet message/capability before changing `Data`, whose v0.1 meaning is echo. Define address assignment, MTU, packet size bounds, IPv4/IPv6 validation and policy for packets claiming an unassigned source. Do not infer packet boundaries from TLS record boundaries.

Build client/server packet adapters in two Linux network namespaces joined by a veth pair. Use an explicit test subnet and a synthetic destination namespace. The first implementation should require opt-in route configuration and leave the host default route untouched. Keep the TLS outer route outside the tunnel to prevent recursion.

Acceptance checklist:

- Transmit ICMP and controlled TCP/UDP flows through TUN in both directions, including IPv6.
- Reject malformed lengths, unsupported IP versions, spoofed source addresses and packets above negotiated MTU.
- Demonstrate DNS queries using a local test resolver and document which resolver routes are installed.
- Test disconnect, reconnect, partial setup failure, process crash and cleanup; prove routes/DNS are restored.
- Record throughput, latency, CPU/memory and loss behavior with a reproducible `netem` setup, before enabling a production default route.

Follow with NAT/routed-subnet deployment modes, ICMP/PMTU behavior, fair queueing, per-device authorization and live session revocation. Keep UDP/IP traffic datagram semantics in the next wire revision; reliable TLS-over-TCP delivery is a baseline with known nested-TCP and head-of-line limitations.

## Versioning policy

Cargo package versions and wire versions are distinct. Breaking wire semantics require an explicit new version or negotiated capability with downgrade behavior documented in the protocol specification. Never repurpose a reserved field, ALPN identifier or existing message in a deployed version. Transport modules must preserve the same identity and authorization checks during fallback. Fallback must not silently disable authentication.
