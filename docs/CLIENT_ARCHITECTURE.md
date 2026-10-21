# Aegis Client: Ionic React, Capacitor and Rust

Decision date: 2026-10-21. The selected interface stack is **Ionic React + TypeScript**, with **Capacitor for Android/iOS**. The same web interface is intended for future desktop shells. The Rust v0.1 component remains an authenticated echo prototype; it does not yet implement an IP tunnel.

## Implemented scaffold versus VPN capability

`apps/ionic/` is the frontend workspace. It provides a responsive Ionic interface, sample server information and a typed bridge boundary. Sample servers are presentation data, not provisioned or tested VPN endpoints. The native VPN plugin and Rust FFI are not implemented. A browser preview, generated native wrapper or successful frontend build does not establish system VPN connectivity.

An ordinary browser/PWA cannot create a system TUN interface, install default routes or apply an OS kill switch. A WebView gains only those additional privileges that an installed native plugin explicitly implements. Ionic components render the UI; Capacitor is the shell/plugin mechanism, not a VPN implementation.

The current Web fallback must return `available: false`, an explicit unsupported state and `tunnelActive: false`. It must reject connection attempts instead of simulating success. Display no invented throughput or latency. A real native bridge may display connected only after OS tunnel setup and the authenticated IP data plane are active. Restoring the UI after reload must query native state rather than remember a prior button click.

## Component ownership

| Layer | Responsibility | Must not own |
| --- | --- | --- |
| Ionic React/TypeScript | Screens, server selection, capability-aware controls, rendering native status | Private keys, packet processing, protocol cryptography |
| Typed bridge | Validated lifecycle commands and status/capabilities | Arbitrary shell commands or filesystem access |
| Kotlin/Swift mobile plugin | OS consent, service/extension lifecycle, secure credential references, Rust FFI | A separate implementation of Aegis messages |
| Rust core | Authentication, session/protocol state, transport selection and future packet forwarding | UI lifecycle assumptions |
| OS packet adapter | TUN/packet flow, routing/DNS setup, rollback and platform protections | Unbounded packet queues |

All packet handling should stay outside the WebView. The GUI must be able to disappear or be recreated without losing track of the system tunnel's real state.

## Existing bridge contract

The scaffold declares the Capacitor plugin name `AegisVpn` in `apps/ionic/src/bridge/aegis.ts`. It currently defines:

| Method | Meaning |
| --- | --- |
| `getCapabilities()` | Availability, platform, kill-switch/split-tunnel support and an optional reason |
| `getStatus()` | Actual lifecycle state, `tunnelActive`, optional server, byte counters or unavailable values |
| `connect({ profileId, serverId })` | Request a connection using a native-managed profile and approved server reference |
| `disconnect()` | Request disconnection and report resulting state |

The initial state vocabulary is `unsupported`, `disconnected`, `connecting`, `connected`, `disconnecting`, `error`. Registration alone is not proof of capability: a future plugin must report capability based on the installed OS module and permissions. Error responses need stable codes and redacted messages. State changes during reconnect, revocation and OS interruption must eventually be delivered to the UI through a specified notification/polling contract.

Do not send certificate private keys, CA signer keys or bearer secrets through React props, browser storage or the bridge status. The native layer resolves `profileId` to protected credentials. `@capacitor/preferences` uses ordinary platform preference storage/PWA localStorage; it is suitable for non-secret preferences, not the credential vault. A future bridge revision must also define serialization for counters beyond JavaScript's safe integer range and reject unexpected command fields.

## Platform adapters to implement

| Platform | Planned shell and native path | Critical work |
| --- | --- | --- |
| Android | Capacitor, Kotlin plugin, `VpnService`, Rust through JNI/FFI | User consent, foreground lifecycle, protect outer transport sockets, handle revocation and network changes |
| iOS | Capacitor host app plus a separate Network Extension packet-tunnel target, Swift/Rust FFI | Entitlements, shared protected profile access, extension IPC/lifecycle, signed packaging |
| Windows | Proposed Tauri desktop shell reusing Ionic assets; Rust service plus Wintun | Privilege separation, route/DNS rollback, driver packaging review |
| macOS Intel/Apple Silicon | Proposed Tauri shell reusing Ionic assets plus native Network Extension | Entitlements, signing/notarization, extension integration, both architecture builds |
| Linux | Proposed Tauri shell reusing Ionic assets; Rust packet adapter and privileged helper | `/dev/net/tun`, narrowly scoped privileges, DNS integration and desktop packaging |

Capacitor's officially supported application targets are Android, iOS and Web. Windows/macOS/Linux entries above are **Aegis architecture proposals**, not capabilities provided by Capacitor and not implemented shells. Tauri renders a frontend alongside a Rust backend; it also does not supply a ready VPN data plane. Reuse the common view code, with separate Capacitor and Tauri bridge implementations rather than pretending a mobile plugin runs on desktop.

On Android, protect every outer transport socket from tunnel routing before forwarding begins; otherwise it can loop into its own VPN. On Apple systems, keep the packet tunnel in the OS extension lifecycle rather than relying on a WebView timer. Device tests must verify routes and DNS for IPv4 and IPv6, failure rollback and reconnection. A kill-switch label is enabled only on platforms with an implemented and tested enforcement mechanism.

## Build prerequisites, not tested platform claims

These requirements were checked in upstream documentation/source on 2026-10-21. Actual project dependency versions belong in `apps/ionic/package-lock.json`; native generated files must agree with that lock. See [VALIDATION.md](VALIDATION.md) for commands actually executed.

| Scope | Current upstream baseline |
| --- | --- |
| Web tooling | Node.js 22+ for Capacitor 8; npm; Ionic 9 supports React 18+ and TypeScript 5.4+ |
| Ionic routing, if introduced | Ionic 9 React integration uses React Router 6; the initial single-screen scaffold does not require a router |
| Android shell | Android Studio 2025.2.1+; JDK 21; Android SDK compile/target 36 and minimum app SDK 24 |
| Android Rust library | Rust target toolchains and Android NDK/cross-build integration still need to be configured and validated |
| iOS shell | macOS, Xcode 26+, Command Line Tools; iOS deployment target 15.0+; Capacitor 8 defaults to Swift Package Manager |
| Apple Rust libraries | Device/simulator target builds, signed extension embedding and FFI still need implementation/testing |
| Proposed Tauri desktop | Native Rust toolchain and OS webview/build prerequisites; on Windows, C++ Build Tools and WebView2 |

Android Studio can supply the JDK; a separately installed JDK must match the generated build. Mobile minimum SDK numbers are upstream baselines, not a promise that every such device supports the future VPN feature set. Recheck requirements when changing major versions.

## Security and updates

Treat WebView code as a privileged control surface: package trusted local assets, constrain content/navigation origins, use a restrictive CSP, validate every native command and expose only necessary methods. Do not expose a general-purpose Rust command dispatcher. A production application must not load a development server or enable live reload.

Store secrets in reviewed native secure storage; allow only opaque profile references in the web layer. Audit IPC authorization and revoke permissions when a user/device loses access. Native protocol/service updates require signed platform packages and an authenticated release process. Reloading web assets cannot update Rust libraries, Android services or Apple extensions. Any future web-asset update mechanism also needs signed manifests, compatible bridge-version checks and rollback rules; it is not a substitute for native signing.

## Implementation order

1. Finish and test the Linux IP-packet capability/TUN data plane described in [ROADMAP.md](ROADMAP.md).
2. Define a stable Rust lifecycle API and FFI ownership/threading/error contracts, including cancellation and bounded queues.
3. Implement Android `VpnService` and the Capacitor plugin; test permission denial, process recreation, socket protection and leak behavior on real devices.
4. Implement a Windows native service and proposed Tauri bridge while preserving the same UI state contract.
5. Add Apple extension adapters and Linux/macOS desktop packaging with native test gates.

UI build/type checks, browser behavior checks, native compilation, device VPN tests and anti-DPI experiments are separate validation categories. None implies the others.

## Licenses and primary references

Ionic Framework and Capacitor are MIT-licensed at the reviewed upstream commits. Preserve their notices when distributing applicable code. Commercial Ionic services/plugins and future added dependencies have separate terms and must be reviewed individually. Aegis' own repository license remains Apache-2.0.

- [Ionic components selected for the UI](https://ionicframework.com/docs/components).
- [Ionic 9 React/TypeScript migration requirements](https://ionicframework.com/docs/updating/9-0).
- [Capacitor 8 environment requirements and supported targets](https://capacitorjs.com/docs/getting-started/environment-setup).
- [Capacitor 8 platform migration requirements](https://capacitorjs.com/docs/updating/8-0).
- [Capacitor plugin mechanism](https://capacitorjs.com/docs/plugins), [creating plugins](https://capacitorjs.com/docs/plugins/creating-plugins), [Preferences storage](https://capacitorjs.com/docs/apis/preferences).
- [Pinned Capacitor Java 21 and SDK settings](https://github.com/ionic-team/capacitor/blob/841017a3c7ddb84c614be389e74a24ea0a629ff5/android/capacitor/build.gradle).
- [Android VpnService including `protect`](https://developer.android.com/reference/android/net/VpnService).
- [Apple NEPacketTunnelProvider](https://developer.apple.com/documentation/networkextension/nepackettunnelprovider).
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
- [Ionic MIT license, commit 584d9731](https://github.com/ionic-team/ionic-framework/blob/584d9731f9a99910b85ebe2ac2f81c5260d54896/LICENSE).
- [Capacitor MIT license, commit 841017a3](https://github.com/ionic-team/capacitor/blob/841017a3c7ddb84c614be389e74a24ea0a629ff5/LICENSE).
