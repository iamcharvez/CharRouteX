use crate::connectivity::probe_adapter;
use chrono::Utc;
use network_core::{
    AdapterInventory, AdapterKind, AdapterSnapshot, IpAddress, LinkStatus, NetworkAdapter,
    NetworkError, RouteEntry,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_INCLUDE_PREFIX, GAA_FLAG_SKIP_ANYCAST,
    GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, GetIfEntry2, GetIpForwardTable2,
    IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL,
    IP_ADAPTER_ADDRESSES_LH, IP_ADAPTER_DHCP_ENABLED, MIB_IF_ROW2, MIB_IPFORWARD_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::{
    IfOperStatusDormant, IfOperStatusDown, IfOperStatusUp,
};
use windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_INET, SOCKET_ADDRESS,
};

pub struct WindowsAdapterInventory;

impl WindowsAdapterInventory {
    pub fn new() -> Self {
        Self
    }

    pub fn snapshot_with_connectivity(&self) -> Result<AdapterSnapshot, NetworkError> {
        let mut snapshot = self.snapshot()?;
        self.enrich_connectivity(&mut snapshot);
        Ok(snapshot)
    }

    pub fn enrich_connectivity(&self, snapshot: &mut AdapterSnapshot) {
        let health = std::thread::scope(|scope| {
            snapshot
                .adapters
                .iter()
                .cloned()
                .map(|adapter| scope.spawn(move || probe_adapter(&adapter)))
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .unwrap_or_else(|_| crate::connectivity::unknown_health())
                })
                .collect::<Vec<_>>()
        });
        for (adapter, health) in snapshot.adapters.iter_mut().zip(health) {
            adapter.connectivity = Some(health);
        }
    }
}

impl Default for WindowsAdapterInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterInventory for WindowsAdapterInventory {
    fn snapshot(&self) -> Result<AdapterSnapshot, NetworkError> {
        let adapters = query_adapters()?;
        let routes = query_routes(&adapters)?;
        Ok(AdapterSnapshot {
            collected_at: Utc::now(),
            adapters,
            routes,
        })
    }
}

fn query_adapters() -> Result<Vec<NetworkAdapter>, NetworkError> {
    let flags = GAA_FLAG_INCLUDE_PREFIX
        | GAA_FLAG_INCLUDE_GATEWAYS
        | GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_MULTICAST;
    let mut byte_len = 0u32;
    // SAFETY: The initial null-buffer call asks Windows for the required allocation size.
    let first = unsafe { GetAdaptersAddresses(0, flags, None, None, &mut byte_len) };
    // Request both IPv4 and IPv6. Retrying handles a concurrent adapter change.
    let mut storage = vec![0u64; (byte_len.max(15_000) as usize).div_ceil(8)];
    let mut buffer_len = (storage.len() * 8) as u32;
    let mut result = unsafe {
        GetAdaptersAddresses(
            0,
            flags,
            None,
            Some(storage.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>()),
            &mut buffer_len,
        )
    };
    if result == ERROR_BUFFER_OVERFLOW.0 {
        storage.resize((buffer_len as usize).div_ceil(8), 0);
        result = unsafe {
            GetAdaptersAddresses(
                0,
                flags,
                None,
                Some(storage.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>()),
                &mut buffer_len,
            )
        };
    }
    if result != ERROR_SUCCESS.0 {
        return Err(NetworkError::Native(format!(
            "GetAdaptersAddresses returned Windows error {result} (sizing result {first})"
        )));
    }

    let mut adapters = Vec::new();
    let mut current = storage.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    while !current.is_null() {
        // SAFETY: `current` traverses the linked records within the live Windows-owned result buffer.
        let native = unsafe { &*current };
        adapters.push(convert_adapter(native)?);
        current = native.Next;
    }
    adapters.sort_by_key(|adapter| (adapter.status != LinkStatus::Up, adapter.interface_index));
    Ok(adapters)
}

fn convert_adapter(native: &IP_ADAPTER_ADDRESSES_LH) -> Result<NetworkAdapter, NetworkError> {
    let interface_index = unsafe { native.Anonymous1.Anonymous.IfIndex };
    let id = if native.AdapterName.is_null() {
        format!("if-{interface_index}")
    } else {
        unsafe { native.AdapterName.to_string() }
            .unwrap_or_else(|_| format!("if-{interface_index}"))
    };
    let name = wide_string(native.FriendlyName).unwrap_or_else(|| id.clone());
    let description = wide_string(native.Description).unwrap_or_default();
    let mut ipv4_addresses = Vec::new();
    let mut ipv6_addresses = Vec::new();

    let mut unicast = native.FirstUnicastAddress;
    while !unicast.is_null() {
        let item = unsafe { &*unicast };
        if let Some(address) = socket_address(&item.Address) {
            let value = IpAddress {
                address,
                prefix_length: item.OnLinkPrefixLength,
            };
            match address {
                IpAddr::V4(_) => ipv4_addresses.push(value),
                IpAddr::V6(_) => ipv6_addresses.push(value),
            }
        }
        unicast = item.Next;
    }

    let mut gateways = Vec::new();
    let mut gateway = native.FirstGatewayAddress;
    while !gateway.is_null() {
        let item = unsafe { &*gateway };
        if let Some(address) = socket_address(&item.Address) {
            gateways.push(address);
        }
        gateway = item.Next;
    }

    let mut dns_servers = Vec::new();
    let mut dns = native.FirstDnsServerAddress;
    while !dns.is_null() {
        let item = unsafe { &*dns };
        if let Some(address) = socket_address(&item.Address) {
            dns_servers.push(address);
        }
        dns = item.Next;
    }

    let row = interface_stats(interface_index);
    let (bytes_sent, bytes_received) = row.as_ref().map_or((0, 0), |r| (r.OutOctets, r.InOctets));
    let status = match native.OperStatus {
        value if value == IfOperStatusUp => LinkStatus::Up,
        value if value == IfOperStatusDown => LinkStatus::Down,
        value if value == IfOperStatusDormant => LinkStatus::Dormant,
        _ => LinkStatus::Unknown,
    };
    let mac_address = format_mac(
        &native.PhysicalAddress,
        native.PhysicalAddressLength as usize,
    );
    let lowered = format!("{name} {description}").to_ascii_lowercase();

    Ok(NetworkAdapter {
        id,
        interface_index,
        name,
        description,
        kind: adapter_kind(native.IfType, &lowered),
        mac_address,
        status,
        transmit_link_speed_bps: normalize_link_speed(native.TransmitLinkSpeed),
        receive_link_speed_bps: normalize_link_speed(native.ReceiveLinkSpeed),
        ipv4_addresses,
        ipv6_addresses,
        gateways,
        dns_servers,
        dhcp_enabled: unsafe { native.Anonymous2.Flags } & IP_ADAPTER_DHCP_ENABLED != 0,
        ipv4_metric: native.Ipv4Metric,
        ipv6_metric: native.Ipv6Metric,
        bytes_sent,
        bytes_received,
        upload_bytes_per_second: None,
        download_bytes_per_second: None,
        connectivity: None,
    })
}

fn normalize_link_speed(value: u64) -> u64 {
    // IP Helper uses ULONG64_MAX when a driver cannot report a link speed.
    if value == u64::MAX { 0 } else { value }
}

fn query_routes(adapters: &[NetworkAdapter]) -> Result<Vec<RouteEntry>, NetworkError> {
    let mut table = std::ptr::null_mut::<MIB_IPFORWARD_TABLE2>();
    let result = unsafe { GetIpForwardTable2(AF_UNSPEC, &mut table) };
    if result != ERROR_SUCCESS {
        return Err(NetworkError::Native(format!(
            "GetIpForwardTable2 returned Windows error {}",
            result.0
        )));
    }
    if table.is_null() {
        return Err(NetworkError::InvalidData(
            "GetIpForwardTable2 returned a null table".into(),
        ));
    }

    struct TableGuard(*mut MIB_IPFORWARD_TABLE2);
    impl Drop for TableGuard {
        fn drop(&mut self) {
            unsafe { FreeMibTable(self.0.cast()) };
        }
    }
    let _guard = TableGuard(table);
    let table_ref = unsafe { &*table };
    let rows = unsafe {
        std::slice::from_raw_parts(table_ref.Table.as_ptr(), table_ref.NumEntries as usize)
    };
    let mut routes = Vec::with_capacity(rows.len());

    for row in rows {
        let Some(destination) = sockaddr_inet(&row.DestinationPrefix.Prefix) else {
            continue;
        };
        let Some(next_hop) = sockaddr_inet(&row.NextHop) else {
            continue;
        };
        let interface_metric = adapters
            .iter()
            .find(|adapter| adapter.interface_index == row.InterfaceIndex)
            .map(|adapter| match destination {
                IpAddr::V4(_) => adapter.ipv4_metric,
                IpAddr::V6(_) => adapter.ipv6_metric,
            })
            .unwrap_or(0);
        let is_default = row.DestinationPrefix.PrefixLength == 0 && destination.is_unspecified();
        routes.push(RouteEntry {
            interface_index: row.InterfaceIndex,
            destination,
            prefix_length: row.DestinationPrefix.PrefixLength,
            next_hop,
            route_metric: row.Metric,
            interface_metric,
            effective_metric: u64::from(row.Metric) + u64::from(interface_metric),
            protocol: row.Protocol.0,
            age_seconds: row.Age,
            is_default,
            is_preferred_default: false,
        });
    }

    for ipv6 in [false, true] {
        let best = routes
            .iter()
            .filter(|route| {
                route.is_default
                    && route.destination.is_ipv6() == ipv6
                    && adapters.iter().any(|adapter| {
                        adapter.interface_index == route.interface_index
                            && adapter.status == LinkStatus::Up
                    })
            })
            .map(|route| route.effective_metric)
            .min();
        if let Some(best) = best {
            for route in routes.iter_mut().filter(|route| {
                route.is_default
                    && route.destination.is_ipv6() == ipv6
                    && route.effective_metric == best
            }) {
                route.is_preferred_default = true;
            }
        }
    }
    routes.sort_by_key(|route| {
        (
            !route.is_default,
            route.effective_metric,
            route.interface_index,
        )
    });
    Ok(routes)
}

fn interface_stats(interface_index: u32) -> Option<MIB_IF_ROW2> {
    let mut row = MIB_IF_ROW2 {
        InterfaceIndex: interface_index,
        ..Default::default()
    };
    let result = unsafe { GetIfEntry2(&mut row) };
    (result == ERROR_SUCCESS).then_some(row)
}

fn wide_string(value: windows::core::PWSTR) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe { value.to_string().ok() }.filter(|value| !value.is_empty())
}

fn socket_address(value: &SOCKET_ADDRESS) -> Option<IpAddr> {
    if value.lpSockaddr.is_null() {
        return None;
    }
    let family = unsafe { (*value.lpSockaddr).sa_family };
    if family == AF_INET && value.iSockaddrLength as usize >= size_of::<SOCKADDR_IN>() {
        let address = unsafe { &*value.lpSockaddr.cast::<SOCKADDR_IN>() };
        let octets = unsafe { address.sin_addr.S_un.S_un_b };
        Some(IpAddr::V4(Ipv4Addr::new(
            octets.s_b1,
            octets.s_b2,
            octets.s_b3,
            octets.s_b4,
        )))
    } else if family == AF_INET6 && value.iSockaddrLength as usize >= size_of::<SOCKADDR_IN6>() {
        let address = unsafe { &*value.lpSockaddr.cast::<SOCKADDR_IN6>() };
        let octets = unsafe { address.sin6_addr.u.Byte };
        Some(IpAddr::V6(Ipv6Addr::from(octets)))
    } else {
        None
    }
}

fn sockaddr_inet(value: &SOCKADDR_INET) -> Option<IpAddr> {
    let family = unsafe { value.si_family };
    if family == AF_INET {
        let address = unsafe { value.Ipv4 };
        let octets = unsafe { address.sin_addr.S_un.S_un_b };
        Some(IpAddr::V4(Ipv4Addr::new(
            octets.s_b1,
            octets.s_b2,
            octets.s_b3,
            octets.s_b4,
        )))
    } else if family == AF_INET6 {
        let address = unsafe { value.Ipv6 };
        Some(IpAddr::V6(Ipv6Addr::from(unsafe {
            address.sin6_addr.u.Byte
        })))
    } else {
        None
    }
}

fn format_mac(bytes: &[u8], length: usize) -> Option<String> {
    if length == 0 || length > bytes.len() {
        return None;
    }
    Some(
        bytes[..length]
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}

fn adapter_kind(if_type: u32, lowered_label: &str) -> AdapterKind {
    if if_type == IF_TYPE_SOFTWARE_LOOPBACK {
        AdapterKind::Loopback
    } else if if_type == IF_TYPE_TUNNEL {
        AdapterKind::Tunnel
    } else if if_type == IF_TYPE_IEEE80211 {
        AdapterKind::Wifi
    } else if ["virtual", "vpn", "hyper-v", "vmware", "tap", "wintun"]
        .iter()
        .any(|token| lowered_label.contains(token))
    {
        AdapterKind::Virtual
    } else if if_type == IF_TYPE_ETHERNET_CSMACD {
        AdapterKind::Ethernet
    } else {
        AdapterKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_mac_addresses() {
        assert_eq!(
            format_mac(&[0, 1, 0xA, 0xFF], 4).as_deref(),
            Some("00:01:0A:FF")
        );
        assert_eq!(format_mac(&[0; 8], 0), None);
    }
    #[test]
    fn classifies_virtual_ethernet_before_physical() {
        assert_eq!(
            adapter_kind(IF_TYPE_ETHERNET_CSMACD, "hyper-v virtual ethernet"),
            AdapterKind::Virtual
        );
    }

    #[test]
    fn treats_the_windows_unknown_link_speed_sentinel_as_unavailable() {
        assert_eq!(normalize_link_speed(u64::MAX), 0);
        assert_eq!(normalize_link_speed(1_000_000_000), 1_000_000_000);
    }
    #[test]
    fn native_inventory_returns_a_snapshot() {
        let snapshot = WindowsAdapterInventory::new()
            .snapshot()
            .expect("Windows adapter query");
        assert!(
            !snapshot.adapters.is_empty(),
            "Windows should report at least loopback interfaces"
        );
        assert!(
            !snapshot.routes.is_empty(),
            "Windows should report a route table"
        );
    }

    #[test]
    fn bounded_connectivity_checks_return_for_every_adapter() {
        let snapshot = WindowsAdapterInventory::new()
            .snapshot_with_connectivity()
            .expect("Windows connectivity snapshot");
        assert!(
            snapshot
                .adapters
                .iter()
                .all(|adapter| adapter.connectivity.is_some())
        );
    }
}
