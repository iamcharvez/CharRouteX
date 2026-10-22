#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use network_protocol::{AddressConfiguration, OperationRequest, OperationState, OperationStatus};
use std::{
    env, fs,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("CharRouteX helper failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args_os().collect::<Vec<_>>();
    if args.len() != 3 || args[1] != "--request" {
        return Err("usage: windows-helper --request <operation.json>".into());
    }
    let request_path = canonical_operation_path(Path::new(&args[2]))?;
    let source = fs::read_to_string(&request_path).map_err(|error| error.to_string())?;
    if source.len() > 256 * 1024 {
        return Err("operation request is too large".into());
    }
    let request: OperationRequest =
        serde_json::from_str(&source).map_err(|error| error.to_string())?;
    request.validate()?;
    let parent = request_path
        .parent()
        .ok_or("operation directory is missing")?;
    let status_path = parent.join(format!("{}.status.json", request.operation_id));
    let confirm_path = parent.join(format!("{}.confirm", request.operation_id));
    write_status(
        &status_path,
        &request,
        OperationState::Pending,
        "Validated operation request",
        None,
    )?;
    if let Err(error) = apply(&request.interface_name, &request.requested) {
        let _ = write_status(&status_path, &request, OperationState::Failed, &error, None);
        return Err(error);
    }
    let deadline = now_ms() + u64::from(request.confirmation_timeout_seconds) * 1000;
    write_status(
        &status_path,
        &request,
        OperationState::Applied,
        "Configuration applied; awaiting confirmation",
        Some(deadline),
    )?;
    while now_ms() < deadline {
        if confirm_path.exists() {
            write_status(
                &status_path,
                &request,
                OperationState::Confirmed,
                "Configuration confirmed",
                None,
            )?;
            let _ = fs::remove_file(&confirm_path);
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    write_status(
        &status_path,
        &request,
        OperationState::RollingBack,
        "Confirmation timed out; restoring backup",
        None,
    )?;
    match apply(&request.interface_name, &request.backup) {
        Ok(()) => write_status(
            &status_path,
            &request,
            OperationState::RolledBack,
            "Backup configuration restored",
            None,
        ),
        Err(error) => {
            let message = format!("rollback failed: {error}");
            let _ = write_status(
                &status_path,
                &request,
                OperationState::Failed,
                &message,
                None,
            );
            Err(message)
        }
    }
}

fn canonical_operation_path(path: &Path) -> Result<PathBuf, String> {
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err("operation request must be JSON".into());
    }
    Ok(path)
}

fn apply(interface: &str, config: &AddressConfiguration) -> Result<(), String> {
    match config {
        AddressConfiguration::Dhcp => {
            netsh([
                "interface",
                "ipv4",
                "set",
                "address",
                &format!("name={interface}"),
                "source=dhcp",
            ])?;
            netsh([
                "interface",
                "ipv4",
                "set",
                "dnsservers",
                &format!("name={interface}"),
                "source=dhcp",
            ])?;
            run_system("ipconfig.exe", ["/renew", interface])?;
        }
        AddressConfiguration::DhcpRenew => {
            run_system("ipconfig.exe", ["/renew", interface])?;
        }
        AddressConfiguration::DhcpRelease => {
            run_system("ipconfig.exe", ["/release", interface])?;
        }
        AddressConfiguration::MetricOnly { interface_metric } => {
            netsh([
                "interface",
                "ipv4",
                "set",
                "interface",
                &format!("interface={interface}"),
                &format!("metric={interface_metric}"),
                "store=persistent",
            ])?;
        }
        AddressConfiguration::Static {
            address,
            prefix_length,
            additional_addresses,
            gateway,
            dns_servers,
            interface_metric,
        } => {
            let mask = prefix_to_mask(*prefix_length);
            let gateway = gateway.map_or_else(|| "none".into(), |value| value.to_string());
            netsh([
                "interface",
                "ipv4",
                "set",
                "address",
                &format!("name={interface}"),
                "source=static",
                &format!("address={address}"),
                &format!("mask={mask}"),
                &format!("gateway={gateway}"),
                &format!("gwmetric={interface_metric}"),
                "store=persistent",
            ])?;
            for additional in additional_addresses {
                let additional_mask = prefix_to_mask(additional.prefix_length);
                netsh([
                    "interface",
                    "ipv4",
                    "add",
                    "address",
                    &format!("name={interface}"),
                    &format!("address={}", additional.address),
                    &format!("mask={additional_mask}"),
                    "store=persistent",
                ])?;
            }
            if let Some(primary) = dns_servers.first() {
                netsh([
                    "interface",
                    "ipv4",
                    "set",
                    "dnsservers",
                    &format!("name={interface}"),
                    "source=static",
                    &format!("address={primary}"),
                    "register=primary",
                    "validate=no",
                ])?;
                if let Some(secondary) = dns_servers.get(1) {
                    netsh([
                        "interface",
                        "ipv4",
                        "add",
                        "dnsservers",
                        &format!("name={interface}"),
                        &format!("address={secondary}"),
                        "index=2",
                        "validate=no",
                    ])?;
                }
                for (index, server) in dns_servers.iter().enumerate().skip(2) {
                    netsh([
                        "interface",
                        "ipv4",
                        "add",
                        "dnsservers",
                        &format!("name={interface}"),
                        &format!("address={server}"),
                        &format!("index={}", index + 1),
                        "validate=no",
                    ])?;
                }
            } else {
                netsh([
                    "interface",
                    "ipv4",
                    "set",
                    "dnsservers",
                    &format!("name={interface}"),
                    "source=dhcp",
                ])?;
            }
            netsh([
                "interface",
                "ipv4",
                "set",
                "interface",
                &format!("interface={interface}"),
                &format!("metric={interface_metric}"),
                "store=persistent",
            ])?;
        }
    }
    Ok(())
}

fn netsh<const N: usize>(args: [&str; N]) -> Result<Output, String> {
    run_system("netsh.exe", args)
}
fn run_system<I, S>(program: &str, args: I) -> Result<Output, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let executable = PathBuf::from(env::var_os("SystemRoot").ok_or("SystemRoot is unavailable")?)
        .join("System32")
        .join(program);
    let output = Command::new(executable)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
fn prefix_to_mask(prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    })
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn write_status(
    path: &Path,
    request: &OperationRequest,
    state: OperationState,
    message: &str,
    deadline: Option<u64>,
) -> Result<(), String> {
    let status = OperationStatus {
        operation_id: request.operation_id,
        state,
        message: message.into(),
        rollback_deadline_unix_ms: deadline,
    };
    let temp = path.with_extension("tmp");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(&status).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(temp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix_masks_are_correct() {
        assert_eq!(prefix_to_mask(24), Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!(prefix_to_mask(32), Ipv4Addr::new(255, 255, 255, 255));
    }
}
