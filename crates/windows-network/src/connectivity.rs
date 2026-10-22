use network_core::{
    ConnectivityHealth, ConnectivityState, LinkStatus, NetworkAdapter, ProbeResult,
};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned, pki_types::ServerName};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::Win32::{Foundation::ERROR_SUCCESS, NetworkManagement::IpHelper::SendARP};

const PROBE_TIMEOUT: Duration = Duration::from_millis(1_500);
const HTTPS_HOST: &str = "www.msftconnecttest.com";
const HTTPS_PATH: &str = "/connecttest.txt";

pub(crate) fn detect_ipv4_conflict(target: Ipv4Addr, source: Option<Ipv4Addr>) -> bool {
    let mut mac = [0u8; 8];
    let mut length = mac.len() as u32;
    let result = unsafe {
        SendARP(
            u32::from_ne_bytes(target.octets()),
            source
                .map(|value| u32::from_ne_bytes(value.octets()))
                .unwrap_or(0),
            mac.as_mut_ptr().cast(),
            &mut length,
        )
    };
    result == ERROR_SUCCESS.0 && length > 0
}

pub(crate) fn unknown_health() -> ConnectivityHealth {
    ConnectivityHealth {
        state: ConnectivityState::Unknown,
        physical_connected: false,
        local_network_reachable: false,
        gateway_reachable: None,
        dns_reachable: false,
        internet_reachable: false,
        degraded: false,
        latency_ms: None,
        packet_loss: None,
        probes: vec![],
    }
}

pub(crate) fn probe_adapter(adapter: &NetworkAdapter) -> ConnectivityHealth {
    let physical_connected = adapter.status == LinkStatus::Up;
    let source = adapter
        .ipv4_addresses
        .iter()
        .map(|value| value.address)
        .find(|address| !address.is_loopback() && !address.is_unspecified())
        .or_else(|| {
            adapter
                .ipv6_addresses
                .iter()
                .map(|value| value.address)
                .find(|address| !address.is_loopback() && !address.is_unspecified())
        });
    let local_network_reachable = physical_connected && source.is_some();
    if !physical_connected || source.is_none() {
        return ConnectivityHealth {
            state: if physical_connected {
                ConnectivityState::LocalOnly
            } else {
                ConnectivityState::Disconnected
            },
            physical_connected,
            local_network_reachable,
            ..unknown_health()
        };
    }
    let source = source.expect("source checked above");
    let mut probes = Vec::new();

    let gateway_reachable = probe_gateway(adapter, source, &mut probes);
    let (dns_reachable, resolved, dns_latency) = probe_dns(adapter, source, &mut probes);

    let endpoints: Vec<SocketAddr> = match source {
        IpAddr::V4(_) => vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 443),
        ],
        IpAddr::V6(_) => ["[2606:4700:4700::1111]:443", "[2001:4860:4860::8888]:443"]
            .iter()
            .filter_map(|value| value.parse().ok())
            .collect(),
    };
    let mut internet_latencies = dns_latency.into_iter().collect::<Vec<_>>();
    let mut tcp_successes = 0usize;
    for (index, endpoint) in endpoints.iter().enumerate() {
        let started = Instant::now();
        let result = connect_bound(source, *endpoint, PROBE_TIMEOUT);
        let latency = started.elapsed().as_secs_f64() * 1000.0;
        if result.is_ok() {
            tcp_successes += 1;
            internet_latencies.push(latency);
        }
        probes.push(ProbeResult {
            name: format!("TCP endpoint {}", index + 1),
            success: result.is_ok(),
            latency_ms: result.is_ok().then_some(latency),
            detail: result
                .map(|_| "Connected".into())
                .unwrap_or_else(|error| error),
        });
    }

    let https_started = Instant::now();
    let https_result = resolved
        .iter()
        .copied()
        .find(|address| address.is_ipv4() == source.is_ipv4())
        .ok_or_else(|| "DNS returned no compatible address".to_string())
        .and_then(|address| probe_https(source, SocketAddr::new(address, 443)));
    let https_latency = https_started.elapsed().as_secs_f64() * 1000.0;
    let https_reachable = https_result.is_ok();
    if https_reachable {
        internet_latencies.push(https_latency);
    }
    probes.push(ProbeResult {
        name: "HTTPS content check".into(),
        success: https_reachable,
        latency_ms: https_reachable.then_some(https_latency),
        detail: https_result.unwrap_or_else(|error| error),
    });

    let internet_reachable = https_reachable || (dns_reachable && tcp_successes > 0);
    let degraded = internet_reachable
        && (!https_reachable || !dns_reachable || tcp_successes < endpoints.len());
    let state = classify_state(
        physical_connected,
        local_network_reachable,
        gateway_reachable,
        internet_reachable,
        degraded,
    );
    let checked_internet = endpoints.len() + 1;
    let successful_internet = tcp_successes + usize::from(https_reachable);
    let packet_loss = (checked_internet > 0)
        .then_some(1.0 - successful_internet as f64 / checked_internet as f64);
    let latency_ms = (!internet_latencies.is_empty())
        .then(|| internet_latencies.iter().sum::<f64>() / internet_latencies.len() as f64);

    ConnectivityHealth {
        state,
        physical_connected,
        local_network_reachable,
        gateway_reachable,
        dns_reachable,
        internet_reachable,
        degraded,
        latency_ms,
        packet_loss,
        probes,
    }
}

fn classify_state(
    physical: bool,
    local: bool,
    gateway: Option<bool>,
    internet: bool,
    degraded: bool,
) -> ConnectivityState {
    if !physical {
        ConnectivityState::Disconnected
    } else if internet && degraded {
        ConnectivityState::Degraded
    } else if internet {
        ConnectivityState::Healthy
    } else if gateway == Some(true) {
        ConnectivityState::GatewayReachable
    } else if local {
        ConnectivityState::LocalOnly
    } else {
        ConnectivityState::Unknown
    }
}

fn probe_gateway(
    adapter: &NetworkAdapter,
    source: IpAddr,
    probes: &mut Vec<ProbeResult>,
) -> Option<bool> {
    let gateway = adapter
        .gateways
        .iter()
        .copied()
        .find(|value| value.is_ipv4() == source.is_ipv4())?;
    let (IpAddr::V4(gateway), IpAddr::V4(source)) = (gateway, source) else {
        probes.push(ProbeResult {
            name: "IPv6 gateway".into(),
            success: false,
            latency_ms: None,
            detail: "IPv6 neighbor reachability is not exposed by the current ARP probe".into(),
        });
        return None;
    };
    let started = Instant::now();
    let mut mac = [0u8; 8];
    let mut length = mac.len() as u32;
    let result = unsafe {
        SendARP(
            u32::from_ne_bytes(gateway.octets()),
            u32::from_ne_bytes(source.octets()),
            mac.as_mut_ptr().cast(),
            &mut length,
        )
    };
    let success = result == ERROR_SUCCESS.0;
    let latency = started.elapsed().as_secs_f64() * 1000.0;
    probes.push(ProbeResult {
        name: "Gateway neighbor".into(),
        success,
        latency_ms: success.then_some(latency),
        detail: if success {
            "Resolved on-link gateway".into()
        } else {
            format!("Windows error {result}")
        },
    });
    Some(success)
}

fn probe_dns(
    adapter: &NetworkAdapter,
    source: IpAddr,
    probes: &mut Vec<ProbeResult>,
) -> (bool, Vec<IpAddr>, Option<f64>) {
    let Some(server) = adapter
        .dns_servers
        .iter()
        .copied()
        .find(|value| value.is_ipv4() == source.is_ipv4())
    else {
        probes.push(ProbeResult {
            name: "DNS resolution".into(),
            success: false,
            latency_ms: None,
            detail: "No compatible DNS server".into(),
        });
        return (false, vec![], None);
    };
    let started = Instant::now();
    let result = dns_query(source, SocketAddr::new(server, 53), HTTPS_HOST);
    let latency = started.elapsed().as_secs_f64() * 1000.0;
    let success = result.as_ref().is_ok_and(|addresses| !addresses.is_empty());
    let detail = match &result {
        Ok(addresses) => format!("Resolved {} address(es)", addresses.len()),
        Err(error) => error.clone(),
    };
    probes.push(ProbeResult {
        name: "DNS resolution".into(),
        success,
        latency_ms: success.then_some(latency),
        detail,
    });
    (
        success,
        result.unwrap_or_default(),
        success.then_some(latency),
    )
}

fn dns_query(source: IpAddr, server: SocketAddr, host: &str) -> Result<Vec<IpAddr>, String> {
    let query_type = if source.is_ipv4() { 1u16 } else { 28u16 };
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u16;
    let mut packet = Vec::with_capacity(64);
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(&0x0100u16.to_be_bytes());
    packet.extend_from_slice(&1u16.to_be_bytes());
    packet.extend_from_slice(&[0; 6]);
    for label in host.split('.') {
        if label.len() > 63 {
            return Err("DNS label is too long".into());
        }
        packet.push(label.len() as u8);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0);
    packet.extend_from_slice(&query_type.to_be_bytes());
    packet.extend_from_slice(&1u16.to_be_bytes());

    let socket = UdpSocket::bind(SocketAddr::new(source, 0)).map_err(|error| error.to_string())?;
    socket
        .set_read_timeout(Some(PROBE_TIMEOUT))
        .map_err(|error| error.to_string())?;
    socket
        .set_write_timeout(Some(PROBE_TIMEOUT))
        .map_err(|error| error.to_string())?;
    socket
        .send_to(&packet, server)
        .map_err(|error| error.to_string())?;
    let mut response = [0u8; 1500];
    let (length, _) = socket
        .recv_from(&mut response)
        .map_err(|error| error.to_string())?;
    parse_dns_response(&response[..length], id, query_type)
}

fn parse_dns_response(data: &[u8], id: u16, query_type: u16) -> Result<Vec<IpAddr>, String> {
    if data.len() < 12 || u16::from_be_bytes([data[0], data[1]]) != id {
        return Err("Invalid DNS response".into());
    }
    let flags = u16::from_be_bytes([data[2], data[3]]);
    if flags & 0x000F != 0 {
        return Err(format!("DNS response code {}", flags & 0x000F));
    }
    let questions = u16::from_be_bytes([data[4], data[5]]) as usize;
    let answers = u16::from_be_bytes([data[6], data[7]]) as usize;
    let mut position = 12usize;
    for _ in 0..questions {
        skip_dns_name(data, &mut position)?;
        position = position
            .checked_add(4)
            .filter(|value| *value <= data.len())
            .ok_or("Truncated DNS question")?;
    }
    let mut addresses = Vec::new();
    for _ in 0..answers {
        skip_dns_name(data, &mut position)?;
        if position + 10 > data.len() {
            return Err("Truncated DNS answer".into());
        }
        let record_type = u16::from_be_bytes([data[position], data[position + 1]]);
        let length = u16::from_be_bytes([data[position + 8], data[position + 9]]) as usize;
        position += 10;
        if position + length > data.len() {
            return Err("Truncated DNS record".into());
        }
        if record_type == query_type && length == 4 {
            addresses.push(IpAddr::V4(Ipv4Addr::new(
                data[position],
                data[position + 1],
                data[position + 2],
                data[position + 3],
            )));
        } else if record_type == query_type && length == 16 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&data[position..position + 16]);
            addresses.push(IpAddr::V6(bytes.into()));
        }
        position += length;
    }
    Ok(addresses)
}

fn skip_dns_name(data: &[u8], position: &mut usize) -> Result<(), String> {
    loop {
        let length = *data.get(*position).ok_or("Truncated DNS name")?;
        if length & 0xC0 == 0xC0 {
            *position = position
                .checked_add(2)
                .filter(|value| *value <= data.len())
                .ok_or("Truncated DNS pointer")?;
            return Ok(());
        }
        *position += 1;
        if length == 0 {
            return Ok(());
        }
        *position = position
            .checked_add(length as usize)
            .filter(|value| *value <= data.len())
            .ok_or("Truncated DNS label")?;
    }
}

fn connect_bound(
    source: IpAddr,
    remote: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream, String> {
    let domain = if source.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(|error| error.to_string())?;
    socket
        .bind(&SockAddr::from(SocketAddr::new(source, 0)))
        .map_err(|error| error.to_string())?;
    socket
        .connect_timeout(&SockAddr::from(remote), timeout)
        .map_err(|error| error.to_string())?;
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    socket
        .set_write_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    Ok(socket.into())
}

fn probe_https(source: IpAddr, remote: SocketAddr) -> Result<String, String> {
    let tcp = connect_bound(source, remote, PROBE_TIMEOUT)?;
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name =
        ServerName::try_from(HTTPS_HOST.to_owned()).map_err(|error| error.to_string())?;
    let connection =
        ClientConnection::new(Arc::new(config), server_name).map_err(|error| error.to_string())?;
    let mut stream = StreamOwned::new(connection, tcp);
    write!(stream, "GET {HTTPS_PATH} HTTP/1.1\r\nHost: {HTTPS_HOST}\r\nConnection: close\r\nUser-Agent: CharRouteX/1.0\r\n\r\n").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let mut response = Vec::new();
    stream
        .take(4096)
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&response);
    if text.starts_with("HTTP/1.1 200") || text.starts_with("HTTP/1.0 200") {
        Ok("TLS and HTTP 200 verified".into())
    } else {
        Err("HTTPS endpoint did not return HTTP 200".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn health_state_prioritizes_internet_and_degradation() {
        assert_eq!(
            classify_state(true, true, Some(true), true, false),
            ConnectivityState::Healthy
        );
        assert_eq!(
            classify_state(true, true, Some(true), true, true),
            ConnectivityState::Degraded
        );
        assert_eq!(
            classify_state(true, true, Some(true), false, false),
            ConnectivityState::GatewayReachable
        );
        assert_eq!(
            classify_state(false, false, None, false, false),
            ConnectivityState::Disconnected
        );
    }
}
