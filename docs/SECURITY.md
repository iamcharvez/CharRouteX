# Security and privilege strategy

## Trust boundaries

The React webview is treated as an untrusted presentation layer. It receives only `core:default`; shell, filesystem, process, and arbitrary network plugins are not installed. Custom behavior is exposed as an explicit list of typed Rust commands. The CSP permits packaged content and Tauri IPC, not arbitrary remote scripts.

The Tauri process runs as the interactive user. Adapter discovery, route inspection, probes, SQLite persistence, policy evaluation, and validation need no elevation. The application never elevates its webview.

## Privileged helper

Windows-changing operations launch the bundled helper using the UAC `runas` boundary. The helper:

- accepts only a versioned, size-bounded `network-protocol` document;
- validates interface names, addresses, prefixes, gateways, DNS lists, metrics, and timeouts;
- invokes fixed Windows system binaries with an argument array and never a command shell;
- captures the previous adapter state before launch;
- applies one semantic operation at a time;
- writes bounded status documents atomically;
- owns a 60-second confirmation deadline and restores the backup on expiry; and
- is protected by a host-wide operation guard against conflicting changes.

Release installers must be code-signed. The local development installers are intentionally identified as unsigned.

## Persistence and logs

SQLite uses WAL mode. Profile imports are schema-versioned, size-limited, and validated. Health data is retained for 30 days. Audit messages avoid passwords and secret material. Wi-Fi passwords are never collected or exported.

## Failover

Automatic policy is disabled by default. Enabling it authorizes only default-route preference changes among explicitly selected adapters. It does not authorize address or DNS changes, does not disable local-only interfaces, and preserves on-link routes. Uplink changes can terminate existing TCP sessions.

The current build uses interactive UAC for automatic-policy route changes. Deployments requiring unattended failover must use a separately installed, signed service with an authenticated named pipe, restrictive ACL, durable rollback journal, and recovery-on-restart; that service mode is not silently emulated by weakening the elevation boundary.

## Remote management

Remote systems are manually enrolled; CharRouteX performs no broadcast or unauthorised scan. Enrollment records a SHA-256 certificate pin and revocation state. Verification requires normal public TLS validation plus an exact leaf-certificate pin. A remote agent must separately authorize configuration rights and own its rollback timer. Merely adding an endpoint never grants mutation access.
