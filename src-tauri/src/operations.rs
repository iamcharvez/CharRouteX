use network_core::{AdapterInventory, AdapterSnapshot};
use network_protocol::{AddressConfiguration, OperationRequest, OperationStatus, PROTOCOL_VERSION};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Manager};
use uuid::Uuid;
use windows_network::WindowsAdapterInventory;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationVerification {
    pub safe_to_confirm: bool,
    pub checks: Vec<VerificationCheck>,
}

pub fn prepare_and_launch(
    app: &AppHandle,
    interface_index: u32,
    requested: AddressConfiguration,
) -> Result<OperationStatus, String> {
    let guard = operation_lock()
        .lock()
        .map_err(|_| "network operation lock was poisoned".to_string())?;
    ensure_no_active_operation(app)?;
    let snapshot = WindowsAdapterInventory::new()
        .snapshot()
        .map_err(|error| error.to_string())?;
    let adapter = snapshot
        .adapters
        .iter()
        .find(|adapter| adapter.interface_index == interface_index)
        .ok_or("selected interface no longer exists")?;
    let backup = match &requested {
        AddressConfiguration::MetricOnly { .. } => AddressConfiguration::MetricOnly {
            interface_metric: adapter.ipv4_metric.max(1),
        },
        _ => backup_configuration(&snapshot, interface_index)?,
    };
    let operation_id = Uuid::new_v4();
    let request = OperationRequest {
        protocol_version: PROTOCOL_VERSION,
        operation_id,
        interface_name: adapter.name.clone(),
        requested,
        backup,
        confirmation_timeout_seconds: 60,
    };
    request.validate()?;
    let directory = operations_directory(app)?;
    let request_path = directory.join(format!("{operation_id}.request.json"));
    atomic_write(
        &request_path,
        &serde_json::to_vec_pretty(&request).map_err(|error| error.to_string())?,
    )?;
    launch_elevated(&helper_path(app)?, &request_path)?;
    drop(guard);
    Ok(OperationStatus {
        operation_id,
        state: network_protocol::OperationState::Pending,
        message: "Waiting for Windows authorization".into(),
        rollback_deadline_unix_ms: None,
    })
}

fn operation_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn ensure_no_active_operation(app: &AppHandle) -> Result<(), String> {
    let directory = operations_directory(app)?;
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = name.strip_suffix(".request.json") else {
            continue;
        };
        let Ok(operation_id) = Uuid::parse_str(id) else {
            continue;
        };
        match read_status(app, operation_id) {
            Ok(status)
                if matches!(
                    status.state,
                    network_protocol::OperationState::Pending
                        | network_protocol::OperationState::Applied
                        | network_protocol::OperationState::RollingBack
                ) =>
            {
                return Err(format!("network operation {operation_id} is still active"));
            }
            Ok(_) => {}
            Err(_) => {
                let recent = entry
                    .metadata()
                    .and_then(|value| value.modified())
                    .ok()
                    .and_then(|value| SystemTime::now().duration_since(value).ok())
                    .is_some_and(|age| age < Duration::from_secs(300));
                if recent {
                    return Err(format!(
                        "network operation {operation_id} is awaiting Windows authorization"
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn read_status(app: &AppHandle, operation_id: Uuid) -> Result<OperationStatus, String> {
    let path = operations_directory(app)?.join(format!("{operation_id}.status.json"));
    let source = fs::read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "operation is still waiting for authorization".into()
        } else {
            error.to_string()
        }
    })?;
    if source.len() > 64 * 1024 {
        return Err("operation status is too large".into());
    }
    let status: OperationStatus =
        serde_json::from_slice(&source).map_err(|error| error.to_string())?;
    if status.operation_id != operation_id {
        return Err("operation status identifier mismatch".into());
    }
    Ok(status)
}

pub fn confirm(app: &AppHandle, operation_id: Uuid) -> Result<(), String> {
    let status = read_status(app, operation_id)?;
    if status.state != network_protocol::OperationState::Applied {
        return Err("only an applied operation can be confirmed".into());
    }
    let path = operations_directory(app)?.join(format!("{operation_id}.confirm"));
    fs::write(path, b"confirmed").map_err(|error| error.to_string())
}

pub fn restore(app: &AppHandle, operation_id: Uuid) -> Result<OperationStatus, String> {
    let status = read_status(app, operation_id)?;
    if status.state != network_protocol::OperationState::Confirmed {
        return Err("only a confirmed operation can be restored".into());
    }
    let path = operations_directory(app)?.join(format!("{operation_id}.request.json"));
    let source = fs::read(path).map_err(|error| error.to_string())?;
    let request: OperationRequest =
        serde_json::from_slice(&source).map_err(|error| error.to_string())?;
    let snapshot = WindowsAdapterInventory::new()
        .snapshot()
        .map_err(|error| error.to_string())?;
    let adapter = snapshot
        .adapters
        .iter()
        .find(|adapter| adapter.name == request.interface_name)
        .ok_or("the original interface is no longer available")?;
    prepare_and_launch(app, adapter.interface_index, request.backup)
}

pub fn verify(app: &AppHandle, operation_id: Uuid) -> Result<OperationVerification, String> {
    let status = read_status(app, operation_id)?;
    if status.state != network_protocol::OperationState::Applied {
        return Err(
            "verification is available only while an applied operation awaits confirmation".into(),
        );
    }
    let source = fs::read(operations_directory(app)?.join(format!("{operation_id}.request.json")))
        .map_err(|error| error.to_string())?;
    let request: OperationRequest =
        serde_json::from_slice(&source).map_err(|error| error.to_string())?;
    let inventory = WindowsAdapterInventory::new();
    let mut snapshot = inventory.snapshot().map_err(|error| error.to_string())?;
    inventory.enrich_connectivity(&mut snapshot);
    let adapter = snapshot
        .adapters
        .iter()
        .find(|adapter| adapter.name == request.interface_name)
        .ok_or("configured interface is no longer present")?;
    let mut checks = Vec::new();
    let mut required_ok = true;
    let mut required = |name: &str, passed: bool, detail: String| {
        required_ok &= passed;
        checks.push(VerificationCheck {
            name: name.into(),
            passed,
            detail,
        });
    };
    match request.requested {
        AddressConfiguration::Dhcp => required(
            "DHCP enabled",
            adapter.dhcp_enabled,
            format!(
                "Windows reports DHCP {}",
                if adapter.dhcp_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
        ),
        AddressConfiguration::DhcpRenew => required(
            "Lease assigned",
            !adapter.ipv4_addresses.is_empty(),
            format!("{} IPv4 address(es) assigned", adapter.ipv4_addresses.len()),
        ),
        AddressConfiguration::DhcpRelease => required(
            "Lease released",
            adapter.ipv4_addresses.is_empty(),
            format!("{} IPv4 address(es) remain", adapter.ipv4_addresses.len()),
        ),
        AddressConfiguration::MetricOnly { interface_metric } => required(
            "Interface metric",
            adapter.ipv4_metric == interface_metric,
            format!(
                "Expected {interface_metric}; Windows reports {}",
                adapter.ipv4_metric
            ),
        ),
        AddressConfiguration::Static {
            address,
            prefix_length,
            additional_addresses,
            gateway,
            dns_servers,
            interface_metric,
        } => {
            let expected = std::iter::once((address, prefix_length))
                .chain(
                    additional_addresses
                        .into_iter()
                        .map(|value| (value.address, value.prefix_length)),
                )
                .collect::<Vec<_>>();
            let addresses_ok = expected.iter().all(|(address, prefix)| {
                adapter.ipv4_addresses.iter().any(|current| {
                    current.address == std::net::IpAddr::V4(*address)
                        && current.prefix_length == *prefix
                })
            });
            required(
                "IPv4 addresses",
                addresses_ok,
                format!(
                    "Expected {} address(es); Windows reports {}",
                    expected.len(),
                    adapter.ipv4_addresses.len()
                ),
            );
            let gateway_ok =
                gateway.is_none_or(|value| adapter.gateways.contains(&std::net::IpAddr::V4(value)));
            required(
                "Default gateway",
                gateway_ok,
                gateway.map_or_else(
                    || "Local-only configuration requested".into(),
                    |value| format!("Expected gateway {value}"),
                ),
            );
            let dns_ok = dns_servers
                .iter()
                .all(|value| adapter.dns_servers.contains(value));
            required(
                "DNS servers",
                dns_ok,
                format!(
                    "Expected {} resolver(s); Windows reports {}",
                    dns_servers.len(),
                    adapter.dns_servers.len()
                ),
            );
            required(
                "Interface metric",
                adapter.ipv4_metric == interface_metric,
                format!(
                    "Expected {interface_metric}; Windows reports {}",
                    adapter.ipv4_metric
                ),
            );
        }
    }
    let health = adapter.connectivity.as_ref();
    checks.push(VerificationCheck {
        name: "Connectivity".into(),
        passed: health.is_some_and(|value| value.local_network_reachable),
        detail: health
            .map(|value| {
                format!(
                    "State {:?}; internet {}; latency {}",
                    value.state,
                    if value.internet_reachable {
                        "reachable"
                    } else {
                        "not verified"
                    },
                    value
                        .latency_ms
                        .map(|item| format!("{item:.0} ms"))
                        .unwrap_or_else(|| "—".into())
                )
            })
            .unwrap_or_else(|| "No health sample available".into()),
    });
    Ok(OperationVerification {
        safe_to_confirm: required_ok,
        checks,
    })
}

fn backup_configuration(
    snapshot: &AdapterSnapshot,
    interface_index: u32,
) -> Result<AddressConfiguration, String> {
    let adapter = snapshot
        .adapters
        .iter()
        .find(|adapter| adapter.interface_index == interface_index)
        .ok_or("selected interface no longer exists")?;
    if adapter.dhcp_enabled {
        return Ok(AddressConfiguration::Dhcp);
    }
    let address = adapter
        .ipv4_addresses
        .iter()
        .find_map(|entry| match entry.address {
            std::net::IpAddr::V4(address) => Some((address, entry.prefix_length)),
            _ => None,
        })
        .ok_or("static adapter has no IPv4 address to back up")?;
    let gateway = adapter.gateways.iter().find_map(|address| match address {
        std::net::IpAddr::V4(address) => Some(*address),
        _ => None,
    });
    Ok(AddressConfiguration::Static {
        address: address.0,
        prefix_length: address.1,
        additional_addresses: adapter
            .ipv4_addresses
            .iter()
            .filter_map(|entry| match entry.address {
                std::net::IpAddr::V4(value) if value != address.0 => {
                    Some(network_protocol::Ipv4AddressConfiguration {
                        address: value,
                        prefix_length: entry.prefix_length,
                    })
                }
                _ => None,
            })
            .collect(),
        gateway,
        dns_servers: adapter.dns_servers.clone(),
        interface_metric: adapter.ipv4_metric.max(1),
    })
}

fn operations_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("operations");
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

fn helper_path(app: &AppHandle) -> Result<PathBuf, String> {
    let installed = app
        .path()
        .resource_dir()
        .map_err(|error| error.to_string())?
        .join("windows-helper.exe");
    if installed.is_file() {
        return Ok(installed);
    }
    let current = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .ok_or("application executable directory is unavailable")?
        .join("windows-helper.exe");
    if current.is_file() {
        Ok(current)
    } else {
        Err("privileged helper executable is not installed".into())
    }
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    fs::write(&temp, contents).map_err(|error| error.to_string())?;
    fs::rename(temp, path).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn launch_elevated(helper: &Path, request: &Path) -> Result<(), String> {
    use windows::{
        Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_HIDE},
        },
        core::{HSTRING, PCWSTR},
    };
    let helper = HSTRING::from(helper.as_os_str());
    let parameters = HSTRING::from(format!("--request \"{}\"", request.display()));
    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            windows::core::w!("runas"),
            PCWSTR(helper.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SW_HIDE,
        )
    };
    if result.0 as isize <= 32 {
        Err(format!(
            "Windows elevation request failed with code {}",
            result.0 as isize
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn launch_elevated(_helper: &Path, _request: &Path) -> Result<(), String> {
    Err("privileged network changes are supported only on Windows".into())
}
