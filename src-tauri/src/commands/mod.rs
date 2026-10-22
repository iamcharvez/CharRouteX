use crate::database::{
    Database, FailoverConfiguration, ProfileApplyItem, ProfileSummary, RemoteHost,
};
use network_core::{
    AdapterSnapshot, ConnectivityHealth, TrafficRateTracker,
    configuration::{
        ExistingIpv4Network, StaticIpv4Configuration, ValidationIssue, ValidationReport,
        ValidationSeverity, validate_static_ipv4,
    },
    failover::{CandidateHealth, FailoverDecision, FailoverEngine},
};
use network_protocol::{AddressConfiguration, OperationStatus};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, State};
use uuid::Uuid;
use windows_network::WindowsAdapterInventory;

pub struct NetworkMonitor {
    inventory: WindowsAdapterInventory,
    runtime: Mutex<MonitorRuntime>,
    paused: AtomicBool,
    health_interval_seconds: AtomicU64,
}

struct MonitorRuntime {
    rates: TrafficRateTracker,
    health: HashMap<String, ConnectivityHealth>,
    health_sampled_at: Option<Instant>,
    failover: FailoverEngine,
    last_snapshot: Option<AdapterSnapshot>,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            inventory: WindowsAdapterInventory::new(),
            runtime: Mutex::new(MonitorRuntime {
                rates: TrafficRateTracker::default(),
                health: HashMap::new(),
                health_sampled_at: None,
                failover: FailoverEngine::default(),
                last_snapshot: None,
            }),
            paused: AtomicBool::new(false),
            health_interval_seconds: AtomicU64::new(10),
        }
    }

    fn snapshot(&self) -> Result<AdapterSnapshot, String> {
        use network_core::AdapterInventory;
        if self.paused.load(Ordering::Relaxed)
            && let Some(snapshot) = self
                .runtime
                .lock()
                .map_err(|_| "network monitor lock was poisoned".to_string())?
                .last_snapshot
                .clone()
        {
            return Ok(snapshot);
        }
        let mut snapshot = self
            .inventory
            .snapshot()
            .map_err(|error| error.to_string())?;
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "network monitor lock was poisoned".to_string())?;
        let refresh_health = runtime.health_sampled_at.is_none_or(|sampled| {
            sampled.elapsed()
                >= Duration::from_secs(
                    self.health_interval_seconds
                        .load(Ordering::Relaxed)
                        .clamp(3, 300),
                )
        });
        if refresh_health {
            self.inventory.enrich_connectivity(&mut snapshot);
            runtime.health = snapshot
                .adapters
                .iter()
                .filter_map(|adapter| {
                    adapter
                        .connectivity
                        .clone()
                        .map(|health| (adapter.id.clone(), health))
                })
                .collect();
            runtime.health_sampled_at = Some(Instant::now());
        } else {
            for adapter in &mut snapshot.adapters {
                adapter.connectivity = runtime.health.get(&adapter.id).cloned();
            }
        }
        runtime.rates.apply(&mut snapshot);
        runtime.last_snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    pub fn invalidate_health(&self) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.health_sampled_at = None;
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
    pub fn set_health_interval(&self, seconds: u64) -> Result<(), String> {
        if !(3..=300).contains(&seconds) {
            return Err("health probe interval must be between 3 and 300 seconds".into());
        }
        self.health_interval_seconds
            .store(seconds, Ordering::Relaxed);
        self.invalidate_health();
        Ok(())
    }

    fn validate_ipv4(
        &self,
        interface_index: u32,
        config: &StaticIpv4Configuration,
    ) -> Result<ValidationReport, String> {
        use network_core::AdapterInventory;
        let snapshot = self
            .inventory
            .snapshot()
            .map_err(|error| error.to_string())?;
        if !snapshot
            .adapters
            .iter()
            .any(|adapter| adapter.interface_index == interface_index)
        {
            return Err(format!("interface {interface_index} no longer exists"));
        }
        let existing = snapshot
            .adapters
            .iter()
            .flat_map(|adapter| {
                adapter.ipv4_addresses.iter().filter_map(move |entry| {
                    let std::net::IpAddr::V4(address) = entry.address else {
                        return None;
                    };
                    Some(ExistingIpv4Network {
                        interface_index: adapter.interface_index,
                        address,
                        prefix_length: entry.prefix_length,
                    })
                })
            })
            .collect::<Vec<_>>();
        let mut report = validate_static_ipv4(config, interface_index, &existing);
        if report.valid {
            let adapter = snapshot
                .adapters
                .iter()
                .find(|adapter| adapter.interface_index == interface_index)
                .expect("interface checked above");
            let current = adapter
                .ipv4_addresses
                .iter()
                .filter_map(|entry| match entry.address {
                    std::net::IpAddr::V4(value) => Some(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let source = current.first().copied();
            let addresses = std::iter::once(config.address).chain(
                config
                    .additional_addresses
                    .iter()
                    .map(|entry| entry.address),
            );
            for (index, address) in addresses
                .enumerate()
                .filter(|(_, address)| !current.contains(address))
            {
                if windows_network::detect_ipv4_conflict(address, source) {
                    report.issues.push(ValidationIssue { field: if index == 0 { "address".into() } else { format!("additionalAddresses.{index}") }, severity: ValidationSeverity::Error, code: "arp_conflict_detected".into(), message: format!("Another device answered for {address} on the local network. Choose a different address or investigate the conflict.") });
                    report.valid = false;
                }
            }
        }
        Ok(report)
    }

    fn evaluate_failover(
        &self,
        snapshot: &AdapterSnapshot,
        configuration: &FailoverConfiguration,
    ) -> FailoverStatus {
        let mut runtime = self
            .runtime
            .lock()
            .expect("network monitor lock was poisoned");
        let preferred = snapshot
            .routes
            .iter()
            .find(|route| route.is_preferred_default)
            .and_then(|route| {
                snapshot
                    .adapters
                    .iter()
                    .find(|adapter| adapter.interface_index == route.interface_index)
            })
            .map(|adapter| adapter.id.clone());
        runtime.failover.set_active_if_unknown(preferred);
        let candidates = configuration
            .candidates
            .iter()
            .filter(|item| item.enabled)
            .filter_map(|item| {
                snapshot
                    .adapters
                    .iter()
                    .find(|adapter| adapter.id == item.adapter_id)
                    .map(|adapter| CandidateHealth {
                        adapter_id: adapter.id.clone(),
                        priority: item.priority,
                        state: adapter
                            .connectivity
                            .as_ref()
                            .map(|health| health.state)
                            .unwrap_or(network_core::ConnectivityState::Unknown),
                    })
            })
            .collect::<Vec<_>>();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let decision = runtime
            .failover
            .evaluate(now, &configuration.policy, &candidates);
        FailoverStatus {
            active_adapter_id: runtime.failover.active().map(str::to_owned),
            decision: match decision {
                FailoverDecision::Stay => None,
                FailoverDecision::Switch { from, to, reason } => {
                    Some(FailoverSwitch { from, to, reason })
                }
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverSwitch {
    from: Option<String>,
    to: String,
    reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverStatus {
    active_adapter_id: Option<String>,
    decision: Option<FailoverSwitch>,
}

/// Read-only inventory command. No elevation is required and no system state is changed.
#[tauri::command]
pub async fn get_network_adapters(
    monitor: State<'_, Arc<NetworkMonitor>>,
    database: State<'_, Database>,
) -> Result<AdapterSnapshot, String> {
    let monitor = Arc::clone(monitor.inner());
    let snapshot = tauri::async_runtime::spawn_blocking(move || monitor.snapshot())
        .await
        .map_err(|error| format!("network monitor task failed: {error}"))??;
    database.record_snapshot(&snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn validate_ipv4_configuration(
    interface_index: u32,
    config: StaticIpv4Configuration,
    monitor: State<'_, Arc<NetworkMonitor>>,
) -> Result<ValidationReport, String> {
    monitor.validate_ipv4(interface_index, &config)
}

#[tauri::command]
pub fn list_network_profiles(database: State<'_, Database>) -> Result<Vec<ProfileSummary>, String> {
    database.list_profiles()
}

#[tauri::command]
pub async fn create_network_profile(
    name: String,
    description: String,
    database: State<'_, Database>,
    monitor: State<'_, Arc<NetworkMonitor>>,
) -> Result<String, String> {
    let monitor = Arc::clone(monitor.inner());
    let snapshot = tauri::async_runtime::spawn_blocking(move || monitor.snapshot())
        .await
        .map_err(|error| error.to_string())??;
    database.create_profile(&name, &description, &snapshot)
}

#[tauri::command]
pub fn delete_network_profile(id: String, database: State<'_, Database>) -> Result<(), String> {
    database.delete_profile(&id)
}

#[tauri::command]
pub fn duplicate_network_profile(
    id: String,
    database: State<'_, Database>,
) -> Result<String, String> {
    database.duplicate_profile(&id)
}

#[tauri::command]
pub fn update_network_profile(
    id: String,
    name: String,
    description: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    database.update_profile(&id, &name, &description)
}

#[tauri::command]
pub async fn get_profile_apply_plan(
    id: String,
    database: State<'_, Database>,
    monitor: State<'_, Arc<NetworkMonitor>>,
) -> Result<Vec<ProfileApplyItem>, String> {
    let monitor = Arc::clone(monitor.inner());
    let snapshot = tauri::async_runtime::spawn_blocking(move || monitor.snapshot())
        .await
        .map_err(|error| error.to_string())??;
    database.profile_apply_plan(&id, &snapshot)
}

#[tauri::command]
pub fn export_network_profile(id: String, database: State<'_, Database>) -> Result<String, String> {
    database.export_profile(&id)
}

#[tauri::command]
pub fn import_network_profile(
    source: String,
    database: State<'_, Database>,
) -> Result<String, String> {
    database.import_profile(&source)
}

#[tauri::command]
pub fn list_activity_events(
    limit: u32,
    database: State<'_, Database>,
) -> Result<Vec<Value>, String> {
    database.list_events(limit)
}

#[tauri::command]
pub fn apply_address_configuration(
    interface_index: u32,
    configuration: AddressConfiguration,
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<OperationStatus, String> {
    let status = crate::operations::prepare_and_launch(&app, interface_index, configuration)?;
    database.audit(
        "configuration",
        "Protected network configuration requested",
        json!({"interfaceIndex": interface_index, "operationId": status.operation_id}),
    )?;
    Ok(status)
}

#[tauri::command]
pub fn get_network_operation_status(
    operation_id: Uuid,
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<OperationStatus, String> {
    let status = crate::operations::read_status(&app, operation_id)?;
    database.record_operation_status(&status)?;
    Ok(status)
}

#[tauri::command]
pub fn confirm_network_operation(
    operation_id: Uuid,
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<(), String> {
    crate::operations::confirm(&app, operation_id)?;
    database.audit(
        "configuration",
        "Network configuration confirmed",
        json!({"operationId": operation_id}),
    )
}

#[tauri::command]
pub fn restore_network_operation(
    operation_id: Uuid,
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<OperationStatus, String> {
    let status = crate::operations::restore(&app, operation_id)?;
    database.audit(
        "configuration",
        "Configuration restoration requested",
        json!({"sourceOperationId": operation_id, "operationId": status.operation_id}),
    )?;
    Ok(status)
}

#[tauri::command]
pub async fn verify_network_operation(
    operation_id: Uuid,
    app: AppHandle,
) -> Result<crate::operations::OperationVerification, String> {
    tauri::async_runtime::spawn_blocking(move || crate::operations::verify(&app, operation_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn get_failover_configuration(
    database: State<'_, Database>,
) -> Result<FailoverConfiguration, String> {
    database.get_failover_configuration()
}

#[tauri::command]
pub fn save_failover_configuration(
    configuration: FailoverConfiguration,
    database: State<'_, Database>,
) -> Result<(), String> {
    database.save_failover_configuration(&configuration)
}

#[tauri::command]
pub async fn get_failover_status(
    monitor: State<'_, Arc<NetworkMonitor>>,
    database: State<'_, Database>,
) -> Result<FailoverStatus, String> {
    let configuration = database.get_failover_configuration()?;
    let monitor_for_snapshot = Arc::clone(monitor.inner());
    let snapshot = tauri::async_runtime::spawn_blocking(move || monitor_for_snapshot.snapshot())
        .await
        .map_err(|error| error.to_string())??;
    Ok(monitor.evaluate_failover(&snapshot, &configuration))
}

#[tauri::command]
pub fn activate_failover_candidate(
    interface_index: u32,
    metric: u32,
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<OperationStatus, String> {
    let status = crate::operations::prepare_and_launch(
        &app,
        interface_index,
        AddressConfiguration::MetricOnly {
            interface_metric: metric,
        },
    )?;
    database.audit("failover", "Failover route change requested", json!({"interfaceIndex": interface_index, "metric": metric, "operationId": status.operation_id}))?;
    Ok(status)
}

#[tauri::command]
pub fn get_health_history(
    adapter_id: String,
    hours: u32,
    database: State<'_, Database>,
) -> Result<Vec<Value>, String> {
    database.health_history(&adapter_id, hours)
}

#[tauri::command]
pub fn list_remote_hosts(database: State<'_, Database>) -> Result<Vec<RemoteHost>, String> {
    database.list_remote_hosts()
}

#[tauri::command]
pub fn enroll_remote_host(
    display_name: String,
    endpoint: String,
    certificate_fingerprint: String,
    database: State<'_, Database>,
) -> Result<String, String> {
    database.enroll_remote_host(&display_name, &endpoint, &certificate_fingerprint)
}

#[tauri::command]
pub fn revoke_remote_host(id: String, database: State<'_, Database>) -> Result<(), String> {
    database.revoke_remote_host(&id)
}

#[tauri::command]
pub async fn probe_remote_host(
    endpoint: String,
    certificate_fingerprint: String,
) -> Result<crate::remote::RemoteProbeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::remote::probe(&endpoint, &certificate_fingerprint)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn set_monitoring_paused(paused: bool, monitor: State<'_, Arc<NetworkMonitor>>) {
    monitor.set_paused(paused);
}

#[tauri::command]
pub fn get_monitoring_paused(monitor: State<'_, Arc<NetworkMonitor>>) -> bool {
    monitor.is_paused()
}

#[tauri::command]
pub fn set_health_probe_interval(
    seconds: u64,
    monitor: State<'_, Arc<NetworkMonitor>>,
) -> Result<(), String> {
    monitor.set_health_interval(seconds)
}

#[tauri::command]
pub fn set_close_to_tray(enabled: bool, preferences: State<'_, crate::AppPreferences>) {
    preferences.set_close_to_tray(enabled);
}
