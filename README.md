# CharRouteX

**Your Network. Your Control.**

CharRouteX is a Windows 10/11 desktop application for native adapter discovery, per-interface connectivity diagnostics, protected IPv4 configuration, route preference, failover policy, profiles, history, auditing, and explicitly enrolled remote-agent endpoints.

## Included functionality

- Dynamic Ethernet, Wi-Fi, USB, VPN, loopback, tunnel, and virtual-adapter discovery through Windows IP Helper APIs.
- Genuine IPv4/IPv6 addresses, prefixes, gateways, DNS, DHCP state, metrics, MAC, link state/speed, routes, and sampled traffic. Traffic is displayed in bits per second.
- Source-bound configured-DNS, TCP, TLS/HTTPS, and IPv4 gateway-neighbor checks with health, latency, and bounded probe-loss estimates.
- DHCP enable/renew/release and validated static IPv4 configuration through a narrow elevated helper.
- Primary plus 15 additional IPv4 addresses, CIDR/subnet-mask editing, four ordered DNS servers, live conflict detection, and post-apply state verification.
- Automatic 60-second rollback unless a change is explicitly confirmed; confirmed changes can be restored.
- Default-route diagnosis, safe interface-metric control, local-subnet preservation, and primary/backup failover policies with hysteresis and cooldown.
- SQLite profiles with capture, edit, duplicate, import/export, adapter matching, and per-adapter restoration.
- Thirty-day diagnostic history and persistent audit events.
- Manual remote-host enrollment, revocation, and certificate-pinned HTTPS agent verification. CharRouteX never performs unauthorised discovery scans.
- Dark/light UI built with Tauri 2, React, TypeScript, Tailwind CSS, Zustand, and Lucide icons.
- Native Windows notification-area operation with close-to-tray, single-instance reopening, pause/resume, refresh, and explicit Exit controls.

Ordinary failover changes Windows route preference. It does not aggregate multiple links. Per-flow load distribution and true bonded bandwidth are separate designs; bonding requires a remote aggregation tunnel.

## Build and test

Prerequisites are Node.js LTS, Rust stable with the MSVC target, Microsoft C++ Build Tools, and WebView2.

```powershell
npm install
npm run check
npm test
cargo test --workspace
npm run tauri dev
```

Build the Windows installers:

```powershell
npm run tauri build
```

Outputs are written to:

```text
target/release/bundle/msi/CharRouteX_1.1.0_x64_en-US.msi
target/release/bundle/nsis/CharRouteX_1.1.0_x64-setup.exe
target/release/charroutex.exe
```

These local builds are unsigned. Follow [release instructions](docs/RELEASE.md) before public distribution.

## Safety model

The desktop and webview remain non-elevated. Only the bundled helper receives UAC elevation, accepts a bounded versioned JSON protocol, validates typed fields, executes fixed system binaries without a command shell, and owns the rollback timer. Conflicting operations are serialized. The webview has no shell or filesystem plugin access.

See [Architecture](docs/ARCHITECTURE.md), [Security](docs/SECURITY.md), [Feasibility](docs/FEASIBILITY.md), and [Release](docs/RELEASE.md).
