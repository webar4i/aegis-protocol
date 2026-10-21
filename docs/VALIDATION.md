# Aegis v0.1 validation — 2026-10-21

This records executed checks, not planned product capabilities. Tested locally
on Linux x86_64 with `rustc 1.99.1 (58cf8f9c3 2026-10-13)` and Cargo 1.99.1.
The manifest declares Rust 1.88; that minimum compiler was not separately run.

## Results

| Check | Executed result |
| --- | --- |
| `cargo build --locked --workspace` | Passed |
| `cargo build --locked --workspace --release` | Passed with repository defaults |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --locked --workspace` | 45 passed, 0 failed: 10 protocol, 12 core, 6 client, 4 transport, 13 integration |
| Real TCP + TLS loopback | TLS 1.3, mutual certificates and Aegis ALPN verified |
| Binary echo | 0, 16,641 and 1,048,576 byte inputs verified exactly |
| IPv6 transport | `[::1]` bind succeeded; actual mTLS echo executed without skipping |
| Identity failures | Wrong server name, untrusted server CA, unauthorized client, missing client certificate rejected |
| ALPN failures | Absent and mismatched ALPN rejected |
| TLS record modification | Relay flipped a ciphertext byte after TLS and stream establishment; server returned TLS decrypt error before echo |
| Multiplexed logical streams | Two interleaved stream IDs preserved distinct binary payloads |
| Protocol/state failures | Oversized/truncated/invalid headers, versions, sequence replay/order, malformed Hello, closed/reused/unopened streams rejected |
| Limits/deadlines | Negotiated frame/stream limits, 16 MiB combined DATA boundary, partial-frame timeout and bounded closure tested |
| Closure | Clean bidirectional TLS notification/EOF; abrupt TCP EOF and trailing application bytes rejected |
| Separate CLI processes | Text and 1 MiB file echo passed; wrong hostname returned exit 1 |
| Process termination | Idle server stopped cleanly with exit 0 on both SIGINT and Unix SIGTERM |
| Dependency audit | Workspace and separate fuzz lockfiles: 0 reported vulnerabilities, 0 warnings |
| Parser fuzz smoke | 6,014,525 executions in 31 seconds; no parser crash with AddressSanitizer; leak detection limitation below |

The CLI smoke test compared binary files byte-for-byte. The deterministic 1 MiB
test payload was `bytes(0..255)` repeated 4,096 times; its SHA-256 was
`fbbab289f7f94b25736c58be46a994c441fd02552cc6022352e3d86d2fab7c83`.
SHA-256 was used only as an additional test observation, not a replacement for
TLS record integrity.

## Dependency scan

`cargo-audit 0.22.2` scanned the final workspace lockfile (87 dependencies)
against RustSec database commit
`4b587f00996ebe5c05a73eaef105ef241e876414` (1,348 advisories). The first scan
identified `time 0.3.45` (RUSTSEC-2026-0009) and an unmaintained
`rustls-pemfile` warning (RUSTSEC-2025-0134). The final graph uses
`time 0.3.55` and the maintained PEM API in `rustls-pki-types`; no advisory was
ignored. `rustls 0.23.46` includes the October 2026 security fixes.

```bash
cargo install cargo-audit --locked --version 0.22.2
cargo audit --deny warnings
cargo audit --deny warnings --file fuzz/Cargo.lock
```

A clean database scan is not an independent implementation audit and can change
as new advisories are published.

## Fuzz execution and environment limits

The target exercises `decode_header`, `Hello::decode` and `Welcome::decode`.
It does not fuzz the entire asynchronous state machine or rustls. Used
`cargo-fuzz 0.13.2`, `libfuzzer-sys 0.4.13` and nightly Rust
`1.101.0-nightly (19833af72 2026-10-20)`.

The initial run completed 7,555,149 inputs without a parser failure, but exited
with a LeakSanitizer infrastructure error: this environment denies the required
`/proc/.../task`/ptrace access. It is **not** counted as a clean sanitizer run.
The following rerun retained AddressSanitizer and disabled only leak detection:

```bash
ASAN_OPTIONS=detect_leaks=0 cargo +nightly fuzz run frame_parser -- -max_total_time=30 -max_len=20000 -seed=2283215844
```

That rerun exited 0 after 6,014,525 executions. Leak checking was not performed.
This short smoke test found no parser crash; it does not prove memory safety,
complete coverage or security. On an ordinary development host and in CI, run
without disabling leak detection and extend fuzz duration substantially.

## Build correction

A release build with an optional custom ThinLTO profile failed with unresolved
standard-library symbols in the local Rust/linker combination. Removing the
premature LTO/codegen tuning restored normal release builds. The committed
profile uses ordinary compiler release defaults plus symbol stripping; the
successful final build required no environment override.

## Not executed / not implemented

- Windows/macOS execution or cross-compilation. CI jobs are supplied, but have
  not run remotely at this checkpoint. Android/iOS/Flutter clients do not exist.
- TUN, IP packet forwarding, routed IPv4/IPv6, real TCP/UDP sessions through a
  VPN, DNS routing/leak tests, kill switch and split tunneling.
- QUIC, HTTP/2, HTTP/3 or MASQUE implementations or comparative experiments.
- Real censorship/DPI tests, availability claims, throughput/latency benchmarks,
  distributed overload tests, exhaustive connection-cap tests or sustained fuzzing.
- Control API, persistent accounting, device revocation or runnable 3x-ui adapter.
- Independent security audit or production deployment.

IPv6 **outer transport** echo is not evidence of IPv6 IP-tunnel support.
The protocol's visible ALPN and TLS behavior are not browser camouflage.

GitHub repository provisioning/publication was still waiting for an authorized
browser session at this checkpoint; source creation and all local checks above
are independent of that external step.
