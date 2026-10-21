# Threat model: experimental v0.1

This model describes the echo prototype, not a production VPN. It is an engineering review aid, not a security audit.

## Assets and trust boundaries

Assets are certificate private keys, the development CA signing key, data exchanged by authenticated peers, and server CPU/memory/file descriptors. Trust boundaries are the unauthenticated network, the TLS peer-verification boundary, the authenticated Aegis parser, the local filesystem and the process administrator.

Assume the peer operating systems and configured CA are trusted, clocks are reasonably correct, entropy is available and secret files are protected. An attacker may observe, inject, delay, replay, reorder or drop network traffic; connect with arbitrary bytes; or hold a valid client certificate and deliberately violate Aegis framing and state rules.

## Threats and controls

| Threat | v0.1 control | Residual limit |
| --- | --- | --- |
| Passive payload interception | TLS 1.3 authenticated encryption | Endpoints see plaintext; packet size, timing and endpoints remain observable |
| Network MITM/server impersonation | Trusted certificate chain plus server-name verification | A compromised trust anchor or endpoint defeats this boundary |
| Unauthorized client | Mandatory TLS client certificate | Every certificate trusted by the configured client CA is admitted; no per-device allowlist or revocation |
| Ciphertext modification/replay | TLS record authentication and record sequence handling; no early data or resumption | Application operations repeated in a fresh authenticated session are not deduplicated |
| Duplicate or reordered Aegis frames | Strict per-direction sequence from zero, checked before accepting a frame | This is state validation, not an independent cryptographic replay mechanism |
| Oversized allocation | Hard 16 KiB frame payload limit checked before allocation; negotiated limits | TLS, OS buffers and bounded concurrent sessions also consume memory |
| Authenticated abusive streams | Maximum negotiated stream count; monotonically increasing odd IDs; strict transitions | Fair scheduling and per-device quotas remain future work |
| Slow/malformed peers | Handshake and frame I/O deadlines, session budgets, bounded connection tasks | Distributed denial of service and repeated handshake CPU exhaustion need operational controls |
| Protocol confusion/downgrade | TLS 1.3 only; exact ALPN; version checked in frames and negotiation | The explicit ALPN itself is a recognizable protocol marker |
| Truncation | Aegis FIN/ACK and CLOSE/ACK distinguish clean completion; incomplete reads fail | A network attacker can always interrupt delivery |

The sequence counter is not reused within a session and exhaustion terminates the connection. Reconnecting creates a new TLS handshake and new application sequence space. No session-resumption token or long-lived Aegis session key exists in v0.1. No attempt is made to conceal client certificates from a malicious server to which the user has intentionally connected.

Current implementation bounds:

| Resource | Bound |
| --- | --- |
| Concurrent server tasks, including TLS handshakes | 64; excess accepted sockets are dropped |
| TCP/TLS connection or TLS accept | 10 seconds |
| First Aegis Hello | 10 seconds |
| Complete Aegis frame read/write | 30 seconds; partial bytes do not reset the deadline |
| Session lifetime | 3,600 seconds |
| Concurrent logical streams | At most 16, or the lower negotiated count |
| Frame payload | At most 16,384 bytes; DATA also respects the lower negotiated size |
| DATA budget | 16 MiB for both directions combined; an echoed byte consumes two bytes |
| Frame count | 65,536 in each direction, including control messages |
| CLI input | 1 MiB |
| Credential PEM input | Ordinary local files, at most 1 MiB each; intermediate PEM key buffer zeroized |
| Process shutdown drain | Ctrl-C or Unix SIGTERM: 5 seconds, then remaining tasks are aborted and reaped |

These are fixed prototype limits, not rate limiting or persistent traffic quotas. They bound one process/session; they do not make a distributed service resistant to denial of service.

## Secrets and certificate lifecycle

Generate a unique client key and certificate per test device. Distribute the CA certificate through an authenticated channel. Keep the CA private key offline after issuance and never deploy it with the server. Store server/client keys in access-controlled files; do not place key material in command-line argument values, source control, logs or screenshots. The CLI accepts file paths. Prototype keys are unencrypted on disk when loaded, so OS permissions are part of the trust assumption.

The bundled rcgen development generator creates one server/device pair and discards the CA signing key rather than saving it. It refuses to overwrite an existing output directory. On Unix it creates the directory with mode 0700 and files with mode 0600; Windows users must protect the directory with their account ACL. These disposable demo credentials are not a credential-management service.

Before a multi-user service, implement explicit device identity mapping, short-lived credentials, rotation overlap, revocation propagation to every node, active-session termination, and CA recovery procedures. Future native clients should use OS secure storage where available.

## Privacy and retained statistics

The prototype does not intentionally log payloads, application messages, destination URLs, DNS queries, certificate identities or peer IP addresses. Process output may include generic connection outcomes and byte/frame totals. Session counters are returned in memory only after successful closure, then discarded by the current server; failed or aborted sessions lose their counters. They cannot support billing or persistent quotas. There is no telemetry export, account table or retention job. A supervising service may retain stdout/stderr and the operating system/network provider may record independent metadata; deployment policy must account for that.

Any future persistent accounting must name each collected field, purpose, retention period and access policy. Prefer aggregate usage counters and coarse failure categories. Do not add payload capture, full packet dumps or per-destination logging to routine diagnostics. Packet captures for research require controlled synthetic traffic and explicit retention boundaries.

## Out of scope and release gates

The prototype does not provide IP routing, Internet access, DNS or IPv6 leak protection, a kill switch, anonymous payment, endpoint anonymity, resistance to traffic correlation or verified censorship resistance. An on-path censor can block an IP address, domain, port, TLS fingerprint or transport entirely. Random padding alone does not establish indistinguishability.

Before public use: sustained parser/state-machine fuzzing with retained corpus, dependency/security-advisory review, negative TLS tests, resource exhaustion tests, audited credential lifecycle, route/DNS rollback tests, platform-specific leak tests and independent protocol/code audit. Test and fuzz harnesses are not evidence that these gates have passed; only documented executions count.

Primary reference: [TLS 1.3 security considerations and 0-RTT replay discussion, RFC 8446](https://www.rfc-editor.org/rfc/rfc8446.html), reviewed 2026-10-21.
