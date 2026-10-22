# Technical feasibility

## Summary

The requested Windows feature set is feasible, with three important boundaries: interface-aware connectivity requires explicitly bound sockets and multiple independent probes; safe configuration requires a privileged service plus rollback; and ordinary Windows route metrics provide preference/failover, not bandwidth bonding.

## Capability assessment

| Capability | Feasibility | Primary mechanism | Main constraint |
| --- | --- | --- | --- |
| Dynamic adapter inventory | High | IP Helper APIs and change notifications | Virtual/VPN classification is partly heuristic |
| IPv4/IPv6 details | High | `GetAdaptersAddresses` | Some driver fields may be absent |
| Live counters/link speed | High | `GetIfEntry2`/`GetIfTable2` | Rates require sampled deltas |
| DHCP/static configuration | High | NetIO/IP Helper/WMI or tightly scoped service implementation | Requires elevation and rollback |
| Per-interface internet checks | High | Bound sockets, DNS, TCP/TLS/HTTP probes | Source/interface binding differs by protocol |
| Route table inspection | High | `GetIpForwardTable2` | Multiple compartments/VPN policy require care |
| Safe default-route preference | High | Validated route/metric operations | Existing sessions may break after uplink switch |
| Automatic failover | High | Health state machine plus route policy | Must prevent flapping and preserve LAN routes |
| Multi-WAN distribution | Medium | Policy routing/per-flow selection | Not equivalent to bonding; application behavior varies |
| True bandwidth bonding | Conditional | Remote aggregation tunnel with split/reassembly | Requires external endpoint and a separate product module |
| Remote management | High | Enrolled agent, mTLS, signed policy, audit | Rollback is mandatory when control path can be lost |
| Cross-platform expansion | Medium | Core traits are portable | Mutation backends and privilege models must be reimplemented |

## Windows support

Windows 10 and 11 expose the required IP Helper/NetIO information. Tauri uses WebView2 for rendering and Rust targets MSVC. Read-only inventory normally works as a standard user. Adapter configuration, route writes, and service installation require administrative authorization.

## Key risks and mitigations

1. **Self-inflicted disconnection.** Persist the exact before-state, apply a narrow transaction, verify connectivity, and automatically restore unless confirmed before a durable deadline.
2. **False internet failure.** Combine gateway, DNS, and multiple TCP/HTTPS signals with consecutive-failure thresholds. Never rely on ICMP or one public endpoint alone.
3. **Route flapping.** Use separate degrade/fail/recovery thresholds, minimum dwell time, cooldown, and preference margins.
4. **Adapter identity drift.** Match durable GUID/hardware facts, record confidence, and ask before applying an ambiguous profile.
5. **Privilege escalation surface.** Keep the desktop process unprivileged; expose semantic, versioned helper operations with strict bounds and authenticated local transport.
6. **Remote lockout.** The remote agent owns the rollback timer, independent of controller connectivity.
7. **Unbounded telemetry.** Aggregate older samples and enforce retention limits.
8. **Driver inconsistency.** Treat absent counters/speeds as unavailable rather than inventing values.

## Product boundaries

- IPv4 gateway reachability uses on-link ARP resolution; IPv6 gateway-neighbor verification is reported as unavailable rather than guessed.
- Probe loss is a bounded application-level estimate across independent TCP/HTTPS checks, not an ICMP packet-loss claim.
- Health probes use documented fixed endpoints and the adapter's configured DNS server.
- Historical availability is an application-level estimate derived from bounded samples; it is not an SLA measurement.
- On-demand protected mutations require interactive UAC. Fully unattended failover requires the separately hardened service transport described in the architecture.
- Remote enrollment and pinned-agent verification are included; the managed computer must run an independently provisioned authorized agent before remote adapter/configuration endpoints can be used.

