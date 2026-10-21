# Aegis v0.1 architecture

Status: experimental echo prototype. The normative wire contract is [PROTOCOL.md](../PROTOCOL.md). This document separates implemented components from the intended VPN ecosystem.

## Implemented boundary

| Component | Responsibility | Excluded from v0.1 |
| --- | --- | --- |
| `protocol/` | Bounded binary framing, version and sequence checks, typed negotiation messages | Cryptography, sockets, OS routing |
| `transports/` | TCP sockets and authenticated TLS 1.3 byte streams | QUIC, HTTP tunneling, traffic camouflage |
| `core/` | Negotiation, stream lifecycle, bounded echo session, timeouts | TUN forwarding, arbitrary proxy destinations |
| `server/` | Listener, concurrency bound, TLS acceptance, graceful process shutdown | Users database, billing, production metrics |
| `client-cli/` | TLS connection, echo exchange and byte equality verification | System VPN, GUI, DNS configuration |
| `apps/ionic/` | Ionic React UI scaffold and typed Capacitor bridge contract | Native VPN plugin, Rust FFI, system packet forwarding |

`core` and `protocol` operate on asynchronous byte streams rather than a concrete socket. This keeps transport selection outside the message state machine. The current session service is echo: opening several logical streams does not create independent congestion control or remove TCP head-of-line blocking. Replacing the echo service with IP forwarding requires a new, specified capability and message type; existing `Data` messages must not silently acquire a different meaning.

## Security and ownership

TLS authenticates both peers using certificates rooted in explicitly configured trust stores. The client additionally verifies the server DNS name. Only TLS 1.3 and ALPN `aegis/0.1` are accepted. Early data, tickets and resumption are disabled for this prototype. Aegis does not define encryption, key derivation, signatures or a second encryption layer. Fresh TLS handshakes produce session keys; rustls owns their lifecycle.

The application protocol owns negotiation, framing, stream identifiers, control messages and session limits. The transport owns confidentiality, record integrity, peer identity verification and reliable byte delivery. In v0.1, every valid client certificate chaining to the configured CA is authorized. There is no individual certificate revocation, account expiry or live policy distribution yet. Use a dedicated development CA, never a broad enterprise CA, for the demo.

## Library choices

| Library family | Reason for selection |
| --- | --- |
| Tokio 1.x | Shared asynchronous I/O, bounded timers, tasks, sockets and signals |
| rustls 0.23.46 or later compatible 0.23 release + tokio-rustls 0.26 | TLS implementation with certificate verification and an asynchronous stream adapter |
| `ring` provider | Established cryptographic implementation available through rustls; selected explicitly to avoid ambiguous provider selection |
| rustls-pki-types (through rustls), zeroize 1.x | Supported PEM parser; regular-file and 1 MiB loading bound; zeroize intermediate private-key PEM buffers |
| clap 4.x | Typed command-line arguments and generated help |
| thiserror 2.x | Explicit component error types without secret-bearing custom diagnostics |
| rcgen 0.13, development/tests only | Generate disposable certificate authorities and device/server certificates without an external certificate tool |

These are dependency families, not a statement that every later release is compatible or independently audited. `Cargo.lock` records the actual resolved versions. Review security advisories and target support when updating them. The bundled development PKI generator uses rcgen and does not require OpenSSL; it deliberately does not persist the CA signing key.

The rustls minimum is 0.23.46 because the RustSec advisories issued 2026-10-14 list that release as patched for RUSTSEC-2026-0346 through RUSTSEC-2026-0349. Those advisories concern session-value debug output, rejected ECH application-data ordering, optional Brotli certificate decompression, and unauthenticated key-share prediction. This prototype disables resumption and does not enable ECH or Brotli, but still adopts the patched release. This is a dependency choice, not a claim that the entire dependency graph has passed an advisory scan. See [actual validation](VALIDATION.md).

## Client stack and planned ecosystem

The selected UI stack is Ionic React with TypeScript. Capacitor supplies the Android/iOS application shell and JavaScript-to-native plugin mechanism. The initial `apps/ionic/` scaffold is a UI and typed bridge boundary; it does not include a native VPN plugin or connect the Rust echo session to OS packet routing. Its Web fallback must report unsupported capability and must not fabricate a connected state.

Native Kotlin/Swift adapters will eventually call a narrow Rust FFI layer for connection lifecycle and statistics and supply packet I/O and privileged route/DNS operations. Protocol parsing, authorization policy, reconnection and transport selection remain in Rust; platform implementations must not recreate them in TypeScript, Kotlin or Swift. The future IP tunnel must be implemented and tested before a graphical client can offer real system VPN connectivity.

Capacitor officially targets Android, iOS and Web. It does not itself supply Windows/macOS/Linux VPN backends. The proposed desktop path is a Tauri shell reusing the same Ionic React web assets and Rust core, with native platform packet/privilege adapters. No desktop shell is implemented at this stage. Details and current build requirements are in [CLIENT_ARCHITECTURE.md](CLIENT_ARCHITECTURE.md).

| Target | Planned OS integration | Separate work required |
| --- | --- | --- |
| Android | `VpnService`, protect the outer transport socket | Background lifecycle, network changes, permission consent |
| iOS/macOS | Network Extension packet tunnel provider | Entitlements, extension lifecycle, packaging and signing |
| Windows 10/11 | TUN through Wintun | Driver redistribution/license review, service permissions, routes |
| Linux | `/dev/net/tun` | Least-privilege helper, routing and DNS integration |

Kill switch, split tunneling, DNS/IPv6 leak protection and signed application updates are platform features with explicit acceptance tests. They are not provided by TLS, Ionic or Capacitor automatically. Apple Intel/Apple Silicon and other targets need native builds and device testing before support can be claimed.

The future Go/PostgreSQL Control API will manage desired policy. A separate authenticated server-management channel will apply policy to Rust nodes. VPN packet traffic must never traverse that API. See [Control API design](../control-api/README.md) and [3x-ui research](../integrations/3x-ui/README.md).

## Primary references

Reviewed 2026-10-21:

- [Tokio documentation](https://docs.rs/tokio/latest/tokio/).
- [rustls 0.23.46 documentation and provider selection](https://docs.rs/rustls/0.23.46/rustls/).
- [tokio-rustls documentation](https://docs.rs/tokio-rustls/latest/tokio_rustls/).
- [TLS 1.3, RFC 8446](https://www.rfc-editor.org/rfc/rfc8446.html).
- [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html), [0347](https://rustsec.org/advisories/RUSTSEC-2026-0347.html), [0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html), [0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html).
