# System architecture

## Design goals

CharRouteX must observe ordinary network state without elevation, make privileged changes only through a narrow audited boundary, support any number of interfaces, and keep platform-specific operations out of the policy engine. The architecture is split into four trust zones.

```text
React webview (untrusted presentation)
        │ typed, allow-listed Tauri invoke
        ▼
Tauri desktop host (standard user, orchestration + read-only queries)
        │ platform-neutral traits
        ├────────► network-core (models, validation, health/routing policy)
        │
        └────────► windows-network (Win32/IP Helper observation)
                         │
                         │ versioned operation document + explicit UAC approval
                         ▼
              Windows helper process (privileged mutations only)
                         │
                         ▼
                Windows networking stack
```

The bundled helper is launched per protected operation through `ShellExecuteW` with the `runas` verb. It accepts only the typed protocol in `network-protocol`, applies the requested semantic operation, and rolls back independently if confirmation is not received within 60 seconds.

## Runtime components

### React presentation

The frontend renders system facts received from Tauri, stores only presentation preferences in browser storage, and never constructs shell commands. `src/services` is the single invocation adapter. Zustand separates persisted settings from non-persisted live state.

### Tauri host

The desktop host runs as the interactive user. It owns application lifecycle, scheduled observation, SQLite access, structured logs, cancellation, and the allow-listed command surface. Read-only operations should remain usable without administrator privileges.

Command groups:

| Command | Direction | Elevation | Side effects |
| --- | --- | --- | --- |
| `get_network_adapters`, history, profiles, remote registry | UI → host | None | Local read/persistence |
| validation and failover evaluation | UI → host | None | Pure planning |
| IP/DHCP/metric apply and restoration | UI → helper | UAC | Protected Windows change |

| Group | Examples | Enforcement |
| --- | --- | --- |
| Read | routes, adapter snapshot, diagnostics, history | Main window capability; bounded inputs/timeouts |
| Plan | validate static config, calculate route changes | Pure functions; no system writes |
| Mutate | apply IP, alter metric/route, renew DHCP | Helper-only; operation ID, backup, approval, rollback |
| Remote mutate | update enrolled endpoint | Mutual authentication, remote approval policy, mandatory rollback |

### `network-core`

Platform-neutral domain types and traits live here, including:

- `addressing`: CIDR/mask validation, gateways, overlaps, address-plan warnings.
- `connectivity`: health state machine and multi-signal scoring.
- `routing`: desired-route planning and local-subnet preservation invariants.
- `failover`: primary/backup policy, hysteresis, cooldown, recovery.
- `profiles`: versioned configuration documents and adapter matching.
- `operations`: immutable plans and transaction states.

No Windows handles, commands, or GUI types may enter this crate.

### `windows-network`

This crate implements discovery with `GetAdaptersAddresses` and `GetIfEntry2`, route inventory with `GetIpForwardTable2`, native interface/route change notifications, and interface-bound probes. Every unsafe block is kept local and converted to owned Rust values before returning.

### Privileged helper

The helper accepts a versioned, size-bounded operation document created by the desktop application after user approval. It exposes semantic operations—never arbitrary command execution. Each request includes an operation ID, interface identity, requested state, exact backup, and a bounded confirmation timeout.

The service flow is:

1. Windows displays the UAC consent boundary and launches the signed helper path.
2. Validate protocol, size, types, interface identity, and timeout.
3. Persist the operation document containing the captured before-state.
4. Apply the smallest requested change via Windows APIs.
5. Start the helper-owned rollback deadline.
6. Publish bounded status documents to the desktop host.
7. Commit only after explicit confirmation or policy-authorized health success.
8. Roll back on expiry or failed verification.

The webview and general desktop host are never elevated. A future unattended service variant must use an authenticated named pipe and a dedicated least-privilege identity; the current on-demand helper intentionally requires UAC for every mutation session.

## Adapter identity

No fixed interface slots exist. Windows adapter IDs are stored as opaque strings and reconciled with interface GUID, LUID/index, MAC address, and hardware description. Interface index alone is not a durable identity. Profiles use matching confidence and require confirmation when hardware identity is ambiguous.

## Monitoring and events

Windows interface and route callbacks trigger debounced immediate snapshots, with configurable polling as reconciliation. A stateful sampler calculates rates from monotonic counter deltas and treats counter reset or interface replacement as a new baseline.

Health is a state machine, not a boolean:

```text
disconnected → local-only → gateway-reachable → internet-healthy
                                      └───────→ degraded
```

Each relevant interface receives independent bounded DNS and TCP/HTTPS probes. A single endpoint failure never declares the internet down. Probe destinations and privacy behavior are documented and configurable.

## Routing invariants

- Never remove on-link/local subnet routes merely because the interface lacks internet.
- Never disable a connected Ethernet adapter as an internet correction.
- Never change a route without an approved manual operation or enabled automatic policy.
- Prefer metric/route changes scoped to default-route candidates.
- Preserve an exact before-state for restoration.
- Serialize conflicting network operations per host.
- Explain that an upstream switch can terminate existing TCP sessions.

Failover changes route preference. It is not bonding. Load distribution and true bandwidth aggregation are separate future designs.

## UI layout

The desktop uses a persistent left navigation grouped into Overview, Control, Tools, and System. A compact header exposes live status and theme control. Dashboard summary cards show only observed values; adapter cards disclose native addresses, gateway, metric, link rate, and counters.

At narrower supported sizes the content remains a two-to-one-column responsive grid while navigation can collapse to icons. The minimum desktop window is 1000×680.

## Database

SQLite runs in WAL mode from the Tauri host. The schema is versioned under `database/migrations`. The webview has no direct filesystem or database access. Write transactions are short, samples are retained/aggregated by policy, and schema changes are forward migrations. Credentials are represented only by references to Windows Credential Manager/DPAPI-protected material.

