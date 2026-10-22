# Product coverage and verification

CharRouteX is delivered as one integrated application; the former phase placeholders have been removed.

## Implemented product areas

- Tauri/React shell, dark/light themes, all eleven navigation areas, genuine Windows data only.
- Native dynamic adapter and route discovery, change notifications, traffic sampling, and interface-aware health checks.
- Static IPv4 validation plus protected DHCP/static/metric changes, explicit confirmation, rollback, and restoration.
- Route diagnosis, local-subnet preservation, manual priorities, and ranked failover policy with failure/recovery thresholds and cooldown.
- SQLite profiles, editing, duplication, import/export, adapter matching, and per-adapter restoration.
- Diagnostic history, activity logs, and bounded retention.
- Explicit remote endpoint enrollment, revocation, and certificate-pinned TLS agent verification.
- MSI, NSIS setup executable, and standalone executable builds.

## Required release qualification

Automated tests cover core validation, rate calculations, failover hysteresis/cooldown, protocol bounds, helper mask conversion, remote URL parsing, native Windows inventory, connectivity checks, and frontend formatters.

Before public distribution, a release owner must still run the physical Windows 10/11 lab matrix: Ethernet/Wi-Fi/USB/VPN/Hyper-V/VirtualBox combinations; DNS and gateway failure; adapter removal; UAC denial; rollback timeout; static-address collision; Wi-Fi roaming; process interruption; upgrade/uninstall; and signed-installer validation. These hardware and certificate checks cannot be truthfully replaced by unit tests on one workstation.

## Optional expansion

- A signed unattended Windows service transport for zero-touch failover deployments.
- Managed-agent adapter/configuration screens after the organization deploys its authorized remote agent.
- Per-flow multi-WAN policy routing.
- True bonding only as a separate tunnel-and-aggregation module with an external endpoint.
- Linux and macOS platform backends behind the existing platform-neutral core contracts.
