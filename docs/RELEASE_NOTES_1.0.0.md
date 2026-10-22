# CharRouteX 1.0.0

Initial integrated Windows release.

## Highlights

- Native unlimited adapter inventory and Windows route-table correlation.
- Independent per-interface gateway, DNS, TCP, and certificate-validated HTTPS health checks.
- Correct real-time throughput presentation in bits per second, distinct from one-shot speed tests.
- Protected DHCP, lease renew/release, static IPv4, DNS, and interface-metric operations.
- Captured before-state, helper-owned 60-second rollback, explicit confirmation, and restoration.
- Ranked failover candidates with consecutive failure/recovery thresholds and cooldown.
- Local-only adapters and on-link routes remain enabled during internet route changes.
- SQLite-backed profiles, adapter matching, import/export, restoration, 30-day history, and audit events.
- Explicit remote-agent enrollment, revocation, and certificate-pinned HTTPS verification.
- MSI and NSIS installers plus a standalone executable.

## Important boundaries

- Installer and executable files produced in this development workspace are unsigned. Sign them before public distribution.
- Automatic route changes use interactive Windows UAC. An unattended deployment requires the hardened service transport described in the architecture document.
- Remote configuration requires an independently provisioned and authorized managed-computer agent; enrollment alone never grants mutation access.
- Failover selects an uplink. It is not load balancing or bonded bandwidth, and existing TCP sessions can disconnect after an upstream change.
