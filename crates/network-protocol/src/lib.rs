use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4AddressConfiguration {
    pub address: Ipv4Addr,
    pub prefix_length: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationRequest {
    pub protocol_version: u32,
    pub operation_id: Uuid,
    pub interface_name: String,
    pub requested: AddressConfiguration,
    pub backup: AddressConfiguration,
    pub confirmation_timeout_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum AddressConfiguration {
    Dhcp,
    DhcpRenew,
    DhcpRelease,
    MetricOnly {
        interface_metric: u32,
    },
    Static {
        address: Ipv4Addr,
        prefix_length: u8,
        #[serde(default)]
        additional_addresses: Vec<Ipv4AddressConfiguration>,
        gateway: Option<Ipv4Addr>,
        dns_servers: Vec<IpAddr>,
        interface_metric: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    Pending,
    Applied,
    Confirmed,
    RollingBack,
    RolledBack,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    pub operation_id: Uuid,
    pub state: OperationState,
    pub message: String,
    pub rollback_deadline_unix_ms: Option<u64>,
}

impl OperationRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err("unsupported helper protocol version".into());
        }
        if self.interface_name.trim().is_empty()
            || self.interface_name.chars().count() > 256
            || self.interface_name.chars().any(char::is_control)
        {
            return Err("invalid interface name".into());
        }
        if !(30..=300).contains(&self.confirmation_timeout_seconds) {
            return Err("confirmation timeout must be between 30 and 300 seconds".into());
        }
        validate_configuration(&self.requested)?;
        validate_configuration(&self.backup)
    }
}

fn validate_configuration(config: &AddressConfiguration) -> Result<(), String> {
    if let AddressConfiguration::MetricOnly { interface_metric } = config {
        if !(1..=9_999).contains(interface_metric) {
            return Err("invalid interface metric".into());
        }
    } else if let AddressConfiguration::Static {
        address,
        prefix_length,
        additional_addresses,
        gateway,
        dns_servers,
        interface_metric,
    } = config
    {
        if *prefix_length == 0
            || *prefix_length > 32
            || additional_addresses
                .iter()
                .any(|value| value.prefix_length == 0 || value.prefix_length > 32)
        {
            return Err("invalid IPv4 prefix length".into());
        }
        if address.is_unspecified()
            || address.is_multicast()
            || address.is_loopback()
            || address.is_broadcast()
        {
            return Err("invalid IPv4 host address".into());
        }
        if additional_addresses.len() > 15
            || additional_addresses.iter().any(|value| {
                value.address.is_unspecified()
                    || value.address.is_multicast()
                    || value.address.is_loopback()
                    || value.address.is_broadcast()
            })
        {
            return Err("invalid additional IPv4 address list".into());
        }
        let mut unique = std::collections::HashSet::new();
        unique.insert(*address);
        if additional_addresses
            .iter()
            .any(|value| !unique.insert(value.address))
        {
            return Err("duplicate IPv4 address in configuration".into());
        }
        if gateway.is_some_and(|value| {
            value.is_unspecified()
                || value.is_multicast()
                || value.is_loopback()
                || value.is_broadcast()
        }) {
            return Err("invalid IPv4 gateway".into());
        }
        if dns_servers.len() > 4
            || dns_servers
                .iter()
                .any(|value| value.is_unspecified() || value.is_multicast())
        {
            return Err("invalid DNS server list".into());
        }
        if !(1..=9_999).contains(interface_metric) {
            return Err("invalid interface metric".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_confirmation_deadlines() {
        let request = OperationRequest {
            protocol_version: PROTOCOL_VERSION,
            operation_id: Uuid::nil(),
            interface_name: "Ethernet".into(),
            requested: AddressConfiguration::Dhcp,
            backup: AddressConfiguration::Dhcp,
            confirmation_timeout_seconds: 5,
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_static_aliases() {
        let address = "192.168.1.20".parse().unwrap();
        let request = OperationRequest {
            protocol_version: PROTOCOL_VERSION,
            operation_id: Uuid::nil(),
            interface_name: "Ethernet".into(),
            confirmation_timeout_seconds: 60,
            requested: AddressConfiguration::Static {
                address,
                prefix_length: 24,
                additional_addresses: vec![Ipv4AddressConfiguration {
                    address,
                    prefix_length: 24,
                }],
                gateway: None,
                dns_servers: vec![],
                interface_metric: 25,
            },
            backup: AddressConfiguration::Dhcp,
        };
        assert!(request.validate().is_err());
    }
}
