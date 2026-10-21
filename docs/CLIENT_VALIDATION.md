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

The example server catalog has no usable endpoints or credentials. Browser
storage contains only non-secret display preferences. The bridge accepts an
opaque profile ID for future native secure storage. Kill switch, split
tunneling, DNS/IPv6 protection, real traffic rates and connection duration are
not simulated as active capabilities.
