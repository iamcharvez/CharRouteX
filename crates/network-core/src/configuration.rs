use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticIpv4Configuration {
    pub address: Ipv4Addr,
    pub prefix_length: u8,
    #[serde(default)]
    pub additional_addresses: Vec<Ipv4AddressConfiguration>,
    pub gateway: Option<Ipv4Addr>,
    pub dns_servers: Vec<IpAddr>,
    pub interface_metric: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4AddressConfiguration {
    pub address: Ipv4Addr,
    pub prefix_length: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingIpv4Network {
    pub interface_index: u32,
    pub address: Ipv4Addr,
    pub prefix_length: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub field: String,
    pub severity: ValidationSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

pub fn validate_static_ipv4(
    config: &StaticIpv4Configuration,
    target_interface: u32,
    existing: &[ExistingIpv4Network],
) -> ValidationReport {
    let mut issues = Vec::new();
    if config.additional_addresses.len() > 15 {
        error(
            &mut issues,
            "additionalAddresses",
            "too_many_addresses",
            "At most 16 IPv4 addresses can be configured in one operation.",
        );
    }
    if config.prefix_length == 0 || config.prefix_length > 32 {
        error(
            &mut issues,
            "prefixLength",
            "invalid_prefix",
            "Prefix length must be between 1 and 32.",
        );
    }
    if config.address.is_unspecified()
        || config.address.is_multicast()
        || config.address.is_broadcast()
        || config.address.is_loopback()
    {
        error(
            &mut issues,
            "address",
            "invalid_host_address",
            "The address cannot be unspecified, multicast, broadcast, or loopback.",
        );
    }
    if config.address.is_link_local() {
        warning(
            &mut issues,
            "address",
            "link_local_static",
            "A static link-local address is unusual and will not provide routed connectivity.",
        );
    }
    if config.interface_metric == 0 || config.interface_metric > 9_999 {
        error(
            &mut issues,
            "interfaceMetric",
            "invalid_metric",
            "Interface metric must be between 1 and 9999.",
        );
    }
    if config.dns_servers.len() > 4 {
        error(
            &mut issues,
            "dnsServers",
            "too_many_dns_servers",
            "At most four DNS servers are supported by this configuration form.",
        );
    }
    let mut configured = vec![Ipv4AddressConfiguration {
        address: config.address,
        prefix_length: config.prefix_length,
    }];
    configured.extend(config.additional_addresses.iter().cloned());
    for (index, candidate) in configured.iter().enumerate() {
        let field = if index == 0 {
            "address".to_string()
        } else {
            format!("additionalAddresses.{index}")
        };
        if candidate.prefix_length == 0 || candidate.prefix_length > 32 {
            error(
                &mut issues,
                &field,
                "invalid_prefix",
                "Prefix length must be between 1 and 32.",
            );
            continue;
        }
        if candidate.address.is_unspecified()
            || candidate.address.is_multicast()
            || candidate.address.is_broadcast()
            || candidate.address.is_loopback()
        {
            error(
                &mut issues,
                &field,
                "invalid_host_address",
                "The address must be a usable unicast IPv4 host address.",
            );
        }
        let network = ipv4_network(candidate.address, candidate.prefix_length);
        let broadcast = ipv4_broadcast(candidate.address, candidate.prefix_length);
        if candidate.prefix_length < 31
            && (candidate.address == network || candidate.address == broadcast)
        {
            error(
                &mut issues,
                &field,
                "network_or_broadcast",
                "The address is the network or broadcast address for this prefix.",
            );
        }
        if configured
            .iter()
            .take(index)
            .any(|other| other.address == candidate.address)
        {
            error(
                &mut issues,
                &field,
                "duplicate_configured_address",
                "The same IPv4 address appears more than once in this plan.",
            );
        }
        for other in existing.iter().filter(|network| {
            network.interface_index != target_interface && network.prefix_length <= 32
        }) {
            if other.address == candidate.address {
                error(
                    &mut issues,
                    &field,
                    "duplicate_local_address",
                    &format!(
                        "This address is already assigned to interface {}.",
                        other.interface_index
                    ),
                );
            } else if networks_overlap(
                candidate.address,
                candidate.prefix_length,
                other.address,
                other.prefix_length,
            ) {
                warning(
                    &mut issues,
                    &field,
                    "overlapping_network",
                    &format!(
                        "This subnet overlaps interface {} ({}/{}).",
                        other.interface_index, other.address, other.prefix_length
                    ),
                );
            }
        }
    }
    for dns in &config.dns_servers {
        if dns.is_unspecified() || dns.is_multicast() {
            error(
                &mut issues,
                "dnsServers",
                "invalid_dns_server",
                "DNS servers must be unicast addresses.",
            );
        }
    }

    if (1..=32).contains(&config.prefix_length) {
        let network = ipv4_network(config.address, config.prefix_length);
        let broadcast = ipv4_broadcast(config.address, config.prefix_length);
        if config.prefix_length < 31 && (config.address == network || config.address == broadcast) {
            error(
                &mut issues,
                "address",
                "network_or_broadcast",
                "The address is the network or broadcast address for this prefix.",
            );
        }
        if let Some(gateway) = config.gateway {
            if gateway.is_unspecified()
                || gateway.is_multicast()
                || gateway.is_broadcast()
                || gateway.is_loopback()
            {
                error(
                    &mut issues,
                    "gateway",
                    "invalid_gateway",
                    "The gateway must be a valid unicast address.",
                );
            } else if !configured
                .iter()
                .any(|candidate| same_subnet(candidate.address, gateway, candidate.prefix_length))
            {
                error(
                    &mut issues,
                    "gateway",
                    "gateway_outside_subnet",
                    "The gateway is outside the configured subnet.",
                );
            } else if gateway == config.address {
                error(
                    &mut issues,
                    "gateway",
                    "gateway_is_host",
                    "The gateway cannot equal the interface address.",
                );
            }
        } else {
            warning(
                &mut issues,
                "gateway",
                "local_only",
                "No gateway is configured; this adapter will be local-only.",
            );
        }
    }
    ValidationReport {
        valid: !issues
            .iter()
            .any(|issue| issue.severity == ValidationSeverity::Error),
        issues,
    }
}

fn error(issues: &mut Vec<ValidationIssue>, field: &str, code: &str, message: &str) {
    issues.push(ValidationIssue {
        field: field.into(),
        severity: ValidationSeverity::Error,
        code: code.into(),
        message: message.into(),
    });
}
fn warning(issues: &mut Vec<ValidationIssue>, field: &str, code: &str, message: &str) {
    issues.push(ValidationIssue {
        field: field.into(),
        severity: ValidationSeverity::Warning,
        code: code.into(),
        message: message.into(),
    });
}
fn mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}
fn ipv4_network(address: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(address) & mask(prefix))
}
fn ipv4_broadcast(address: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(address) | !mask(prefix))
}
fn same_subnet(a: Ipv4Addr, b: Ipv4Addr, prefix: u8) -> bool {
    u32::from(a) & mask(prefix) == u32::from(b) & mask(prefix)
}
fn networks_overlap(a: Ipv4Addr, a_prefix: u8, b: Ipv4Addr, b_prefix: u8) -> bool {
    let prefix = a_prefix.min(b_prefix);
    same_subnet(a, b, prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> StaticIpv4Configuration {
        StaticIpv4Configuration {
            address: "192.168.50.10".parse().unwrap(),
            prefix_length: 24,
            additional_addresses: vec![],
            gateway: Some("192.168.50.1".parse().unwrap()),
            dns_servers: vec!["1.1.1.1".parse().unwrap()],
            interface_metric: 25,
        }
    }

    #[test]
    fn accepts_a_valid_configuration() {
        let report = validate_static_ipv4(&valid(), 1, &[]);
        assert!(report.valid, "{:?}", report.issues);
    }

    #[test]
    fn rejects_out_of_subnet_gateway_and_duplicate_address() {
        let mut config = valid();
        config.gateway = Some("10.0.0.1".parse().unwrap());
        let existing = [ExistingIpv4Network {
            interface_index: 2,
            address: config.address,
            prefix_length: 24,
        }];
        let report = validate_static_ipv4(&config, 1, &existing);
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "gateway_outside_subnet")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "duplicate_local_address")
        );
    }

    #[test]
    fn warns_for_overlapping_networks_and_local_only() {
        let mut config = valid();
        config.gateway = None;
        let existing = [ExistingIpv4Network {
            interface_index: 2,
            address: "192.168.50.200".parse().unwrap(),
            prefix_length: 25,
        }];
        let report = validate_static_ipv4(&config, 1, &existing);
        assert!(report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "overlapping_network")
        );
        assert!(report.issues.iter().any(|issue| issue.code == "local_only"));
    }

    #[test]
    fn validates_multiple_addresses_and_gateway_on_an_alias_subnet() {
        let mut config = valid();
        config.additional_addresses = vec![Ipv4AddressConfiguration {
            address: "10.20.30.40".parse().unwrap(),
            prefix_length: 24,
        }];
        config.gateway = Some("10.20.30.1".parse().unwrap());
        config.dns_servers = vec![
            "1.1.1.1".parse().unwrap(),
            "8.8.8.8".parse().unwrap(),
            "9.9.9.9".parse().unwrap(),
        ];
        assert!(validate_static_ipv4(&config, 1, &[]).valid);
        config.additional_addresses.push(Ipv4AddressConfiguration {
            address: config.address,
            prefix_length: 24,
        });
        let report = validate_static_ipv4(&config, 1, &[]);
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "duplicate_configured_address")
        );
    }
}
