# 3x-ui integration research

Status: source review and bridge design only. There is no working 3x-ui plugin or synchronization adapter in Aegis v0.1.

## Verified upstream facts

Reviewed 2026-10-21 against upstream `MHSanaei/3x-ui`, commit `a8d0700bf2ed06d77c077fc45fc0387d9abdbaab` (main branch observed at review time).

- Upstream describes 3x-ui as a Go web control panel for Xray-core, with additional protocol implementations.
- `go.mod` declares Go, Gin, GORM, SQLite/PostgreSQL drivers and an Xray-core dependency. Do not assume that current versions use only SQLite.
- `internal/xray/process.go` locates platform-specific Xray executables and configuration files and controls Xray process state.
- `internal/web/runtime/runtime.go` contains an internal Go `Runtime` interface for inbound/user mutations, statistics resets and `RestartXray`. An internal interface is not a public dynamic-plugin ABI.
- The repository's `LICENSE` contains GNU GPL version 3. Review the notices and obligations applicable to any code copied into a distributed adapter/fork before distribution.

No supported mechanism for loading an arbitrary external Aegis protocol plugin was established by this review. This is not proof that no extension mechanism can ever exist. It means a plugin cannot be promised without a documented upstream contract and a compatible implementation.

## Proposed independent bridge

Keep `aegis-server` and Aegis Control API independent of Xray-core. Run an eventual bridge as a separate process with narrowly scoped panel/API credentials. The bridge maintains explicit panel-client → Aegis-user/device mappings and calls Aegis APIs. It must not claim that registering an unknown protocol name in an Xray inbound adds protocol support.

| Administrator action | Planned Aegis operation | Correctness requirement |
| --- | --- | --- |
| Register Aegis node | Create server record and enroll node identity | Verify node ownership and management authentication |
| Add client | Create user/device and approve enrollment | Idempotent mapping; device key stays on device |
| Set quota/expiry | Apply policy generation | Obtain node acknowledgement and enforce expiry live |
| View usage | Read reconciled counters | Distinguish upstream panel counters from Aegis accounting |
| Disable/delete client | Revoke policy and terminate sessions | Bounded propagation with retry/reconciliation |
| Export configuration | Generate versioned Aegis configuration | Deliver privately; never embed issuer secrets |

A stock 3x-ui UI may require a separate Aegis management screen or an upstream change/fork to expose these actions. A future fork must pin an upstream release, preserve its license notices and receive maintenance/security review. A separate process communicating via documented APIs is a useful architectural boundary; it is not a blanket legal conclusion about licensing.

Before building the bridge: pin a 3x-ui version, inspect its authenticated API/OpenAPI contracts and scopes, define source-of-truth and conflict behavior, write fake-panel contract tests, and validate revocation/accounting failure scenarios. Avoid scraping the UI and direct database writes.

## Primary source links

- [Repository README](https://github.com/MHSanaei/3x-ui/blob/a8d0700bf2ed06d77c077fc45fc0387d9abdbaab/README.md).
- [License](https://github.com/MHSanaei/3x-ui/blob/a8d0700bf2ed06d77c077fc45fc0387d9abdbaab/LICENSE).
- [Go module](https://github.com/MHSanaei/3x-ui/blob/a8d0700bf2ed06d77c077fc45fc0387d9abdbaab/go.mod).
- [Xray process implementation](https://github.com/MHSanaei/3x-ui/blob/a8d0700bf2ed06d77c077fc45fc0387d9abdbaab/internal/xray/process.go).
- [Internal runtime interface](https://github.com/MHSanaei/3x-ui/blob/a8d0700bf2ed06d77c077fc45fc0387d9abdbaab/internal/web/runtime/runtime.go).
