//! Platform-neutral network inventory contracts used by the UI and future engines.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::IpAddr};

pub mod configuration;
pub mod failover;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdapterKind {
    Ethernet,
    Wifi,
    Loopback,
    Tunnel,
    Virtual,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkStatus {
    Up,
    Down,
    Dormant,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectivityState {
    Disconnected,
    LocalOnly,
    GatewayReachable,
    Healthy,
    Degraded,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub name: String,
    pub success: bool,
    pub latency_ms: Option<f64>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectivityHealth {
    pub state: ConnectivityState,
    pub physical_connected: bool,
    pub local_network_reachable: bool,
    pub gateway_reachable: Option<bool>,
    pub dns_reachable: bool,
    pub internet_reachable: bool,
    pub degraded: bool,
    pub latency_ms: Option<f64>,
    pub packet_loss: Option<f64>,
    pub probes: Vec<ProbeResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpAddress {
    pub address: IpAddr,
    pub prefix_length: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAdapter {
    pub id: String,
    pub interface_index: u32,
    pub name: String,
    pub description: String,
    pub kind: AdapterKind,
    pub mac_address: Option<String>,
    pub status: LinkStatus,
    pub transmit_link_speed_bps: u64,
    pub receive_link_speed_bps: u64,
    pub ipv4_addresses: Vec<IpAddress>,
    pub ipv6_addresses: Vec<IpAddress>,
    pub gateways: Vec<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
    pub dhcp_enabled: bool,
    pub ipv4_metric: u32,
    pub ipv6_metric: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub upload_bytes_per_second: Option<u64>,
    pub download_bytes_per_second: Option<u64>,
    pub connectivity: Option<ConnectivityHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteEntry {
    pub interface_index: u32,
    pub destination: IpAddr,
    pub prefix_length: u8,
    pub next_hop: IpAddr,
    pub route_metric: u32,
    pub interface_metric: u32,
    pub effective_metric: u64,
    pub protocol: i32,
    pub age_seconds: u32,
    pub is_default: bool,
    pub is_preferred_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterSnapshot {
    pub collected_at: DateTime<Utc>,
    pub adapters: Vec<NetworkAdapter>,
    pub routes: Vec<RouteEntry>,
}

#[derive(Debug, Default)]
pub struct TrafficRateTracker {
    previous_at: Option<DateTime<Utc>>,
    previous: HashMap<String, (u64, u64)>,
}

impl TrafficRateTracker {
    pub fn apply(&mut self, snapshot: &mut AdapterSnapshot) {
        let elapsed_ms = self
            .previous_at
            .map(|value| (snapshot.collected_at - value).num_milliseconds())
            .unwrap_or(0);

        for adapter in &mut snapshot.adapters {
            adapter.upload_bytes_per_second = None;
            adapter.download_bytes_per_second = None;
            if elapsed_ms > 0
                && let Some((sent, received)) = self.previous.get(&adapter.id)
            {
                adapter.upload_bytes_per_second = adapter
                    .bytes_sent
                    .checked_sub(*sent)
                    .map(|delta| delta.saturating_mul(1000) / elapsed_ms as u64);
                adapter.download_bytes_per_second = adapter
                    .bytes_received
                    .checked_sub(*received)
                    .map(|delta| delta.saturating_mul(1000) / elapsed_ms as u64);
            }
        }

        self.previous = snapshot
            .adapters
            .iter()
            .map(|adapter| {
                (
                    adapter.id.clone(),
                    (adapter.bytes_sent, adapter.bytes_received),
                )
            })
            .collect();
        self.previous_at = Some(snapshot.collected_at);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("network inventory is not supported on this platform")]
    UnsupportedPlatform,
    #[error("native network API failed: {0}")]
    Native(String),
    #[error("native network API returned malformed data: {0}")]
    InvalidData(String),
}

pub trait AdapterInventory: Send + Sync {
    fn snapshot(&self) -> Result<AdapterSnapshot, NetworkError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    #[test]
    fn adapter_kind_serializes_for_typescript() {
        assert_eq!(
            serde_json::to_string(&AdapterKind::Wifi).unwrap(),
            "\"wifi\""
        );
    }

    #[test]
    fn traffic_rates_use_counter_deltas_and_ignore_resets() {
        let at = Utc::now();
        let mut tracker = TrafficRateTracker::default();
        let mut first = fixture_snapshot(at, 1_000, 2_000);
        tracker.apply(&mut first);
        assert_eq!(first.adapters[0].upload_bytes_per_second, None);

        let mut second = fixture_snapshot(at + TimeDelta::seconds(2), 2_000, 6_000);
        tracker.apply(&mut second);
        assert_eq!(second.adapters[0].upload_bytes_per_second, Some(500));
        assert_eq!(second.adapters[0].download_bytes_per_second, Some(2_000));

        let mut reset = fixture_snapshot(at + TimeDelta::seconds(4), 10, 20);
        tracker.apply(&mut reset);
        assert_eq!(reset.adapters[0].upload_bytes_per_second, None);
    }

    fn fixture_snapshot(collected_at: DateTime<Utc>, sent: u64, received: u64) -> AdapterSnapshot {
        AdapterSnapshot {
            collected_at,
            routes: vec![],
            adapters: vec![NetworkAdapter {
                id: "adapter".into(),
                interface_index: 1,
                name: "Adapter".into(),
                description: String::new(),
                kind: AdapterKind::Ethernet,
                mac_address: None,
                status: LinkStatus::Up,
                transmit_link_speed_bps: 0,
                receive_link_speed_bps: 0,
                ipv4_addresses: vec![],
                ipv6_addresses: vec![],
                gateways: vec![],
                dns_servers: vec![],
                dhcp_enabled: true,
                ipv4_metric: 25,
                ipv6_metric: 25,
                bytes_sent: sent,
                bytes_received: received,
                upload_bytes_per_second: None,
                download_bytes_per_second: None,
                connectivity: None,
            }],
        }
    }
}
