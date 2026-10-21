# Ionic client validation — 2026-10-21

The user-selected interface stack is Ionic React + Capacitor. Flutter is no
longer the planned UI. This checkpoint is a real, executable UI shell with
Android/iOS project sources and an explicit native bridge contract. It does
not contain a native VPN plugin or forward IP packets.

## Executed checks

| Check | Result |
| --- | --- |
| Node/npm | Node 24.19.0, npm 11.9.0 |
| `npm run build` | TypeScript and Vite production build passed |
| `npm test` | 5 tests passed: unsupported bridge, truthful active status, status validation, preference handling |
| `npm audit --audit-level=moderate` | 0 vulnerabilities |
| Capacitor | Android/iOS project generation and `cap sync` passed |
| GitHub Ionic CI | Web tests/build/audit and Java 21 Android `assembleDebug` passed for commit `174ca19e8cf52dc8941d186019e7405ae5ed8546` |
| Desktop browser render | Production assets rendered at 1440 × 1000; no JavaScript page errors |
| Mobile browser render | Production assets rendered at 390 × 844; horizontal scroll width equals viewport width |
| Connection control | Opens the honest unavailable-module dialog; no connected/protected state is manufactured |
| Server selection | Selecting the Germany example updates the connection screen |
| Navigation | Connection, server catalog, settings, and modal dismissal exercised |

The browser checks used Playwright with an isolated Chromium 153 binary, local
production assets, synthetic example server labels and no account credentials.
Screenshots show the current UI, not an image-generated design proposal.
Standard Playwright browser downloads were unavailable in the execution
environment; a Chromium package and its bundled fonts were used for rendering.
The font configuration was adjusted for that isolated test environment only.

The initial npm audit identified moderate `uuid` advisories inherited through
the Capacitor CLI's `xcode` dependency. A narrow `xcode → uuid 11.1.1` override
was applied. Parsing the generated Xcode project and `generateUuid()` were
checked; platform generation, build and tests continued to pass. No advisory
was ignored.

## Native build boundaries

Local Android APK compilation was not performed: this host has Java 17 and no
Android SDK, while the generated project requires Java 21 and SDK 36. There is
no Xcode/iOS signing environment on this Linux host. Source generation and
asset synchronization do not prove that an APK or IPA has been compiled.

The [Ionic CI run](https://github.com/webar4i/aegis-protocol/actions/runs/39575619412)
completed successfully: UI checks and the actual Android Gradle build passed.
The `aegis-android-ui-shell-debug` artifact contains a 4,519,021-byte APK with
SHA-256 `a2a4d86295091f3f52173493ee57c71e4eb766af4f06f0ccdf249aff5ea4d212`.
Artifact download and APK container structure were checked. Installation,
on-device runtime, iOS compilation/signing, desktop native shells and VPN
operation remain unimplemented/unverified at this checkpoint. This is a debug
UI shell, not a functioning Android VPN.

## Linux desktop development shell

Added an independent Tauri 2.12.4 crate and CLI 2.12.3, reusing the existing
Ionic assets. Desktop Rust MSRV is 1.90; the portable protocol workspace
retains its separate lock and requirements. No custom Tauri commands or
native VPN adapter are implemented. Runtime capabilities grant no IPC
permissions; release assets are local and the dev server binds loopback.

`npm ci`, 5 UI tests, production web build, locked desktop Cargo metadata,
Tauri configuration parsing and Rust formatting passed. A user-local installer
and Arch build instructions are in `platforms/linux/` and
[ARCH_LINUX.md](ARCH_LINUX.md). Installer tests use temporary directories;
they do not install into the execution host's real home.

The native release build **passed on Ubuntu 24.04 x86_64**, using Rust 1.99.1,
WebKitGTK 2.52.3 and GTK 3.24.41. The first unconstrained build encountered a
zero-length compiler object while archiving `tauri-utils`; retrying with
`CARGO_BUILD_JOBS=2` completed successfully. The installer defaults to two
Cargo jobs unless overridden. All 9 installer tests passed, including a real
`desktop-file-validate` check and GLib `gio launch` of a temporary fixture
whose path contains spaces and shell/desktop metacharacters. This fixture
launch verifies desktop quoting, not native WebView rendering.

The desktop GUI itself was not launched: this execution host runs as root
without a user's graphical session. No WebKit sandbox bypass was used.
Installation on the user's Arch and actual compositor/graphics behavior
remain to be checked there. Existing browser checks above cover the shared
Ionic UI, not WebKitGTK compatibility.

NPM audit reports 0 vulnerabilities. Desktop `cargo audit --json` reports
0 vulnerability advisories **and two unresolved warnings** inherited through
Tauri's GTK3 stack: `glib 0.18.5` has RUSTSEC-2024-0429 (unsound iterator API),
and `proc-macro-error 1.0.4` has RUSTSEC-2024-0370 (unmaintained). These are
not suppressed or presented as a clean desktop security audit. Updating glib
across its GTK binding version boundary requires an upstream framework change
or a reviewed compatible patch. This preview is for development, and the
desktop dependency review remains open before production distribution.

The execution host is Ubuntu 24.04, not the user's Arch machine. Compilation,
desktop runtime/device checks, user-machine installation and VPN operation
are separate claims. The Arch CI job builds a bare x86_64 binary in an Arch
container; it does not test Wayland/X11 interaction or OS packet forwarding.

The example server catalog has no usable endpoints or credentials. Browser
storage contains only non-secret display preferences. The bridge accepts an
opaque profile ID for future native secure storage. Kill switch, split
tunneling, DNS/IPv6 protection, real traffic rates and connection duration are
not simulated as active capabilities.
