use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned, pki_types::ServerName};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProbeResult {
    pub reachable: bool,
    pub authenticated: bool,
    pub latency_ms: f64,
    pub message: String,
}

pub fn probe(endpoint: &str, expected_fingerprint: &str) -> Result<RemoteProbeResult, String> {
    let (host, port, path) = parse_https_endpoint(endpoint)?;
    let started = Instant::now();
    let address = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or("remote endpoint did not resolve")?;
    let tcp = TcpStream::connect_timeout(&address, Duration::from_secs(4))
        .map_err(|error| format!("connection failed: {error}"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(4)))
        .map_err(|error| error.to_string())?;
    tcp.set_write_timeout(Some(Duration::from_secs(4)))
        .map_err(|error| error.to_string())?;
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from(host.clone()).map_err(|error| error.to_string())?;
    let connection =
        ClientConnection::new(Arc::new(config), name).map_err(|error| error.to_string())?;
    let mut stream = StreamOwned::new(connection, tcp);
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: application/json\r\nUser-Agent: CharRouteX/1.0\r\n\r\n").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let certificate = stream
        .conn
        .peer_certificates()
        .and_then(|values| values.first())
        .ok_or("remote endpoint did not present a certificate")?;
    let actual = format!("{:X}", Sha256::digest(certificate.as_ref()));
    let expected: String = expected_fingerprint
        .chars()
        .filter(|value| *value != ':' && !value.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect();
    if actual != expected {
        return Err(format!("certificate pin mismatch (received {actual})"));
    }
    let mut response = Vec::new();
    stream
        .take(64 * 1024)
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let first = String::from_utf8_lossy(&response)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let authenticated = first.contains(" 200 ");
    Ok(RemoteProbeResult {
        reachable: true,
        authenticated,
        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
        message: if authenticated {
            "Pinned TLS agent identity verified".into()
        } else {
            format!("Agent returned {first}")
        },
    })
}

fn parse_https_endpoint(value: &str) -> Result<(String, u16, String), String> {
    let rest = value
        .strip_prefix("https://")
        .ok_or("remote endpoint must use HTTPS")?;
    let (authority, suffix) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() || authority.contains('@') {
        return Err("invalid HTTPS endpoint authority".into());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(']') => (
            host.to_owned(),
            port.parse::<u16>().map_err(|_| "invalid endpoint port")?,
        ),
        _ => (authority.to_owned(), 443),
    };
    let path = if suffix.is_empty() {
        "/charroutex/v1/status".into()
    } else {
        format!("/{suffix}")
    };
    Ok((host, port, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_https_endpoints() {
        assert_eq!(
            parse_https_endpoint("https://agent.example:9443/status").unwrap(),
            ("agent.example".into(), 9443, "/status".into())
        );
        assert!(parse_https_endpoint("http://agent.example").is_err());
    }
}
