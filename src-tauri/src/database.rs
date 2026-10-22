use chrono::Utc;
use network_core::{AdapterSnapshot, ConnectivityState, failover::FailoverPolicy};
use network_protocol::{AddressConfiguration, OperationState, OperationStatus};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub adapter_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverCandidateConfiguration {
    pub adapter_id: String,
    pub priority: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverConfiguration {
    pub automatic: bool,
    pub policy: FailoverPolicy,
    pub candidates: Vec<FailoverCandidateConfiguration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHost {
    pub id: String,
    pub display_name: String,
    pub endpoint: String,
    pub certificate_fingerprint: String,
    pub enrolled_at: String,
    pub last_seen_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileApplyItem {
    pub adapter_name: String,
    pub interface_index: Option<u32>,
    pub route_role: String,
    pub configuration: Option<AddressConfiguration>,
    pub warning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileExport {
    schema_version: u32,
    name: String,
    description: String,
    adapters: Vec<ProfileAdapterExport>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileAdapterExport {
    adapter_match: Value,
    address_mode: String,
    address_config: Value,
    route_role: String,
    priority: Option<u32>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .map_err(|error| error.to_string())?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| error.to_string())?;
        if version == 0 {
            connection
                .execute_batch(include_str!("../../database/migrations/0001_initial.sql"))
                .map_err(|error| error.to_string())?;
            connection
                .pragma_update(None, "user_version", 1)
                .map_err(|error| error.to_string())?;
        } else if version > 1 {
            return Err(format!(
                "database schema version {version} is newer than this application supports"
            ));
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn list_profiles(&self) -> Result<Vec<ProfileSummary>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let mut statement = connection.prepare("SELECT p.id, p.name, p.description, COUNT(a.id), p.created_at, p.updated_at FROM profiles p LEFT JOIN profile_adapters a ON a.profile_id = p.id GROUP BY p.id ORDER BY p.updated_at DESC").map_err(|error| error.to_string())?;
        statement
            .query_map([], |row| {
                Ok(ProfileSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    adapter_count: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn create_profile(
        &self,
        name: &str,
        description: &str,
        snapshot: &AdapterSnapshot,
    ) -> Result<String, String> {
        validate_profile_text(name, description)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        transaction.execute("INSERT INTO profiles (id, name, description, schema_version, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)", params![id, name.trim(), description.trim(), now]).map_err(|error| friendly_db_error(error, "profile name already exists"))?;
        for adapter in &snapshot.adapters {
            let adapter_id = Uuid::new_v4().to_string();
            let adapter_match = json!({ "systemId": adapter.id, "interfaceIndex": adapter.interface_index, "macAddress": adapter.mac_address, "description": adapter.description });
            let address_config = json!({ "ipv4Addresses": adapter.ipv4_addresses, "ipv6Addresses": adapter.ipv6_addresses, "gateways": adapter.gateways, "dnsServers": adapter.dns_servers, "interfaceMetric": adapter.ipv4_metric });
            let address_mode = if adapter.dhcp_enabled {
                "dhcp"
            } else {
                "static"
            };
            let route_role = if adapter
                .connectivity
                .as_ref()
                .is_some_and(|health| health.internet_reachable)
            {
                "eligible"
            } else {
                "local_only"
            };
            transaction.execute("INSERT INTO profile_adapters (id, profile_id, adapter_match_json, address_mode, address_config_json, route_role, priority) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![adapter_id, id, adapter_match.to_string(), address_mode, address_config.to_string(), route_role, adapter.ipv4_metric]).map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        drop(connection);
        self.audit(
            "profile",
            "Profile created",
            json!({ "profileId": id, "name": name.trim() }),
        )?;
        Ok(id)
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let changed = connection
            .execute("DELETE FROM profiles WHERE id = ?1", [id])
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            return Err("profile not found".into());
        }
        drop(connection);
        self.audit("profile", "Profile deleted", json!({ "profileId": id }))
    }

    pub fn duplicate_profile(&self, id: &str) -> Result<String, String> {
        let export = self.load_export(id)?;
        let mut suffix = 1u32;
        loop {
            let candidate = if suffix == 1 {
                format!("{} Copy", export.name)
            } else {
                format!("{} Copy {suffix}", export.name)
            };
            let mut copy = ProfileExport {
                name: candidate,
                ..clone_export(&export)
            };
            match self.insert_export(&mut copy) {
                Ok(id) => return Ok(id),
                Err(error) if error.contains("already exists") => suffix += 1,
                Err(error) => return Err(error),
            }
        }
    }

    pub fn update_profile(&self, id: &str, name: &str, description: &str) -> Result<(), String> {
        validate_profile_text(name, description)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let changed = connection
            .execute(
                "UPDATE profiles SET name=?2, description=?3, updated_at=?4 WHERE id=?1",
                params![id, name.trim(), description.trim(), Utc::now().to_rfc3339()],
            )
            .map_err(|error| friendly_db_error(error, "profile name already exists"))?;
        if changed == 0 {
            return Err("profile not found".into());
        }
        drop(connection);
        self.audit(
            "profile",
            "Profile details updated",
            json!({"profileId": id}),
        )
    }

    pub fn profile_apply_plan(
        &self,
        id: &str,
        snapshot: &AdapterSnapshot,
    ) -> Result<Vec<ProfileApplyItem>, String> {
        let export = self.load_export(id)?;
        export
            .adapters
            .into_iter()
            .map(|entry| {
                let system_id = entry.adapter_match.get("systemId").and_then(Value::as_str);
                let mac = entry
                    .adapter_match
                    .get("macAddress")
                    .and_then(Value::as_str);
                let adapter = snapshot
                    .adapters
                    .iter()
                    .find(|adapter| system_id == Some(adapter.id.as_str()))
                    .or_else(|| {
                        snapshot
                            .adapters
                            .iter()
                            .find(|adapter| mac.is_some() && adapter.mac_address.as_deref() == mac)
                    });
                let adapter_name = adapter
                    .map(|value| value.name.clone())
                    .or_else(|| {
                        entry
                            .adapter_match
                            .get("description")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "Unmatched adapter".into());
                let Some(adapter) = adapter else {
                    return Ok(ProfileApplyItem {
                        adapter_name,
                        interface_index: None,
                        route_role: entry.route_role,
                        configuration: None,
                        warning: Some("No current adapter matches the stored identity".into()),
                    });
                };
                let configuration = match entry.address_mode.as_str() {
                    "dhcp" => Some(AddressConfiguration::Dhcp),
                    "unchanged" => None,
                    "static" => Some(parse_stored_static(&entry.address_config)?),
                    _ => return Err("profile contains an invalid address mode".into()),
                };
                Ok(ProfileApplyItem {
                    adapter_name,
                    interface_index: Some(adapter.interface_index),
                    route_role: entry.route_role,
                    configuration,
                    warning: None,
                })
            })
            .collect()
    }

    pub fn export_profile(&self, id: &str) -> Result<String, String> {
        serde_json::to_string_pretty(&self.load_export(id)?).map_err(|error| error.to_string())
    }

    pub fn import_profile(&self, source: &str) -> Result<String, String> {
        if source.len() > 1_000_000 {
            return Err("profile document exceeds 1 MB".into());
        }
        let mut export: ProfileExport = serde_json::from_str(source)
            .map_err(|error| format!("invalid profile JSON: {error}"))?;
        if export.schema_version != 1 {
            return Err(format!(
                "unsupported profile schema {}",
                export.schema_version
            ));
        }
        if export.adapters.len() > 256 {
            return Err("profile has too many adapter records".into());
        }
        export.name = format!("{} Imported", export.name);
        self.insert_export(&mut export)
    }

    pub fn list_events(&self, limit: u32) -> Result<Vec<Value>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let mut statement = connection.prepare("SELECT id, occurred_at, severity, category, message, details_json FROM audit_events ORDER BY occurred_at DESC LIMIT ?1").map_err(|error| error.to_string())?;
        statement.query_map([limit.min(500)], |row| {
            let details: String = row.get(5)?;
            Ok(json!({ "id": row.get::<_, String>(0)?, "occurredAt": row.get::<_, String>(1)?, "severity": row.get::<_, String>(2)?, "category": row.get::<_, String>(3)?, "message": row.get::<_, String>(4)?, "details": serde_json::from_str::<Value>(&details).unwrap_or(Value::Null) }))
        }).map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
    }

    pub fn record_operation_status(&self, status: &OperationStatus) -> Result<(), String> {
        let state = match status.state {
            OperationState::Pending => "planned",
            OperationState::Applied => "applied",
            OperationState::Confirmed => "confirmed",
            OperationState::RollingBack => "rolling_back",
            OperationState::RolledBack => "rolled_back",
            OperationState::Failed => "failed",
        };
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let previous: Option<String> = connection
            .query_row(
                "SELECT state FROM network_operations WHERE id=?1",
                [status.operation_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO network_operations (id, host_id, kind, state, requested_at, rollback_deadline, plan_json, result_json) VALUES (?1, 'local', 'network-configuration', ?2, ?3, ?4, '{}', ?5) ON CONFLICT(id) DO UPDATE SET state=excluded.state, rollback_deadline=excluded.rollback_deadline, result_json=excluded.result_json", params![status.operation_id.to_string(), state, Utc::now().to_rfc3339(), status.rollback_deadline_unix_ms.map(|value| value.to_string()), json!({"message": status.message}).to_string()]).map_err(|error| error.to_string())?;
        drop(connection);
        if previous.as_deref() != Some(state) {
            let severity = if matches!(status.state, OperationState::Failed) {
                "error"
            } else if matches!(
                status.state,
                OperationState::RolledBack | OperationState::RollingBack
            ) {
                "warning"
            } else {
                "info"
            };
            let connection = self
                .connection
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?;
            connection.execute("INSERT INTO audit_events (id, occurred_at, severity, category, actor, host_id, operation_id, message, details_json) VALUES (?1, ?2, ?3, 'configuration', 'local-helper', 'local', ?4, ?5, '{}')", params![Uuid::new_v4().to_string(), Utc::now().to_rfc3339(), severity, status.operation_id.to_string(), status.message]).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn record_snapshot(&self, snapshot: &AdapterSnapshot) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        for adapter in &snapshot.adapters {
            transaction.execute(
                "INSERT INTO adapter_identities (id, platform, system_id, interface_guid, mac_address, last_friendly_name, hardware_description, first_seen_at, last_seen_at) VALUES (?1, 'windows', ?1, ?1, ?2, ?3, ?4, ?5, ?5) ON CONFLICT(platform, system_id) DO UPDATE SET mac_address=excluded.mac_address, last_friendly_name=excluded.last_friendly_name, hardware_description=excluded.hardware_description, last_seen_at=excluded.last_seen_at",
                params![adapter.id, adapter.mac_address, adapter.name, adapter.description, snapshot.collected_at.to_rfc3339()],
            ).map_err(|error| error.to_string())?;
            let Some(health) = &adapter.connectivity else {
                continue;
            };
            let last: Option<String> = transaction.query_row(
                "SELECT sampled_at FROM health_samples WHERE adapter_id=?1 ORDER BY sampled_at DESC LIMIT 1",
                [&adapter.id], |row| row.get(0),
            ).optional().map_err(|error| error.to_string())?;
            let recent = last
                .as_deref()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .is_some_and(|value| {
                    (snapshot.collected_at - value.with_timezone(&Utc)).num_seconds() < 10
                });
            if recent {
                continue;
            }
            let state = match health.state {
                ConnectivityState::Disconnected => "disconnected",
                ConnectivityState::LocalOnly => "local_only",
                ConnectivityState::GatewayReachable => "gateway",
                ConnectivityState::Healthy => "healthy",
                ConnectivityState::Degraded => "degraded",
                ConnectivityState::Unknown => "unknown",
            };
            transaction.execute(
                "INSERT INTO health_samples (adapter_id, sampled_at, state, latency_ms, packet_loss, bytes_sent_delta, bytes_received_delta, details_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![adapter.id, snapshot.collected_at.to_rfc3339(), state, health.latency_ms, health.packet_loss, adapter.upload_bytes_per_second.map(|value| value.min(i64::MAX as u64) as i64), adapter.download_bytes_per_second.map(|value| value.min(i64::MAX as u64) as i64), serde_json::to_string(&health.probes).map_err(|error| error.to_string())?],
            ).map_err(|error| error.to_string())?;
        }
        transaction
            .execute(
                "DELETE FROM health_samples WHERE sampled_at < datetime('now', '-30 days')",
                [],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn health_history(&self, adapter_id: &str, hours: u32) -> Result<Vec<Value>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let cutoff =
            (Utc::now() - chrono::Duration::hours(i64::from(hours.clamp(1, 720)))).to_rfc3339();
        let mut statement = connection.prepare("SELECT sampled_at, state, latency_ms, packet_loss, bytes_sent_delta, bytes_received_delta FROM health_samples WHERE adapter_id=?1 AND sampled_at>=?2 ORDER BY sampled_at ASC LIMIT 5000").map_err(|error| error.to_string())?;
        statement.query_map(params![adapter_id, cutoff], |row| Ok(json!({
            "sampledAt": row.get::<_, String>(0)?, "state": row.get::<_, String>(1)?,
            "latencyMs": row.get::<_, Option<f64>>(2)?, "packetLoss": row.get::<_, Option<f64>>(3)?,
            "uploadBytesPerSecond": row.get::<_, Option<i64>>(4)?, "downloadBytesPerSecond": row.get::<_, Option<i64>>(5)?
        }))).map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
    }

    pub fn get_failover_configuration(&self) -> Result<FailoverConfiguration, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key='failover'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        value
            .map(|source| serde_json::from_str(&source).map_err(|error| error.to_string()))
            .transpose()
            .map(|value| value.unwrap_or_default())
    }

    pub fn save_failover_configuration(&self, value: &FailoverConfiguration) -> Result<(), String> {
        validate_failover(value)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        connection.execute("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('failover', ?1, ?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json, updated_at=excluded.updated_at", params![serde_json::to_string(value).map_err(|error| error.to_string())?, Utc::now().to_rfc3339()]).map_err(|error| error.to_string())?;
        drop(connection);
        self.audit("failover", if value.automatic { "Automatic failover enabled" } else { "Failover policy saved" }, json!({"automatic": value.automatic, "candidateCount": value.candidates.iter().filter(|item| item.enabled).count()}))
    }

    pub fn list_remote_hosts(&self) -> Result<Vec<RemoteHost>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let mut statement = connection.prepare("SELECT id, display_name, endpoint, certificate_fingerprint, enrolled_at, last_seen_at, revoked_at IS NOT NULL FROM remote_hosts ORDER BY display_name").map_err(|error| error.to_string())?;
        statement
            .query_map([], |row| {
                Ok(RemoteHost {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    endpoint: row.get(2)?,
                    certificate_fingerprint: row.get(3)?,
                    enrolled_at: row.get(4)?,
                    last_seen_at: row.get(5)?,
                    revoked: row.get(6)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn enroll_remote_host(
        &self,
        display_name: &str,
        endpoint: &str,
        certificate_fingerprint: &str,
    ) -> Result<String, String> {
        validate_remote_host(display_name, endpoint, certificate_fingerprint)?;
        let id = Uuid::new_v4().to_string();
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        connection.execute("INSERT INTO remote_hosts (id, display_name, endpoint, certificate_fingerprint, credential_reference, authorization_policy_json, enrolled_at) VALUES (?1, ?2, ?3, ?4, '', '{\"read\":true,\"configure\":false}', ?5)", params![id, display_name.trim(), endpoint.trim(), normalize_fingerprint(certificate_fingerprint), Utc::now().to_rfc3339()]).map_err(|error| friendly_db_error(error, "remote endpoint is already enrolled"))?;
        drop(connection);
        self.audit(
            "remote",
            "Remote host enrolled",
            json!({"hostId": id, "endpoint": endpoint.trim()}),
        )?;
        Ok(id)
    }

    pub fn revoke_remote_host(&self, id: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let changed = connection
            .execute(
                "UPDATE remote_hosts SET revoked_at=?2 WHERE id=?1 AND revoked_at IS NULL",
                params![id, Utc::now().to_rfc3339()],
            )
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            return Err("active remote host not found".into());
        }
        drop(connection);
        self.audit(
            "remote",
            "Remote host authorization revoked",
            json!({"hostId": id}),
        )
    }

    pub(crate) fn audit(
        &self,
        category: &str,
        message: &str,
        details: Value,
    ) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        connection.execute("INSERT INTO audit_events (id, occurred_at, severity, category, actor, message, details_json) VALUES (?1, ?2, 'info', ?3, 'local-user', ?4, ?5)", params![Uuid::new_v4().to_string(), Utc::now().to_rfc3339(), category, message, details.to_string()]).map_err(|error| error.to_string())?;
        Ok(())
    }

    fn load_export(&self, id: &str) -> Result<ProfileExport, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let profile = connection
            .query_row(
                "SELECT name, description, schema_version FROM profiles WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u32>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or("profile not found")?;
        let mut statement = connection.prepare("SELECT adapter_match_json, address_mode, address_config_json, route_role, priority FROM profile_adapters WHERE profile_id = ?1 ORDER BY id").map_err(|error| error.to_string())?;
        let adapters = statement
            .query_map([id], |row| {
                let adapter_match: String = row.get(0)?;
                let address_config: String = row.get(2)?;
                Ok(ProfileAdapterExport {
                    adapter_match: serde_json::from_str(&adapter_match).unwrap_or(Value::Null),
                    address_mode: row.get(1)?,
                    address_config: serde_json::from_str(&address_config).unwrap_or(Value::Null),
                    route_role: row.get(3)?,
                    priority: row.get(4)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        Ok(ProfileExport {
            schema_version: profile.2,
            name: profile.0,
            description: profile.1,
            adapters,
        })
    }

    fn insert_export(&self, export: &mut ProfileExport) -> Result<String, String> {
        validate_profile_text(&export.name, &export.description)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        transaction.execute("INSERT INTO profiles (id, name, description, schema_version, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)", params![id, export.name.trim(), export.description.trim(), now]).map_err(|error| friendly_db_error(error, "profile name already exists"))?;
        for adapter in &export.adapters {
            if !matches!(
                adapter.address_mode.as_str(),
                "dhcp" | "static" | "unchanged"
            ) || !matches!(
                adapter.route_role.as_str(),
                "local_only" | "primary" | "backup" | "eligible" | "unchanged"
            ) {
                return Err("profile contains an invalid adapter mode or route role".into());
            }
            transaction.execute("INSERT INTO profile_adapters (id, profile_id, adapter_match_json, address_mode, address_config_json, route_role, priority) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![Uuid::new_v4().to_string(), id, adapter.adapter_match.to_string(), adapter.address_mode, adapter.address_config.to_string(), adapter.route_role, adapter.priority]).map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(id)
    }
}

fn validate_failover(value: &FailoverConfiguration) -> Result<(), String> {
    if !(1..=10).contains(&value.policy.failure_threshold)
        || !(1..=10).contains(&value.policy.recovery_threshold)
    {
        return Err("failover health thresholds must be between 1 and 10".into());
    }
    if !(5..=3600).contains(&value.policy.cooldown_seconds) {
        return Err("failover cooldown must be between 5 and 3600 seconds".into());
    }
    if value.candidates.len() > 64 {
        return Err("too many failover candidates".into());
    }
    let mut ids = std::collections::HashSet::new();
    for candidate in &value.candidates {
        if candidate.adapter_id.trim().is_empty() || !ids.insert(candidate.adapter_id.clone()) {
            return Err("failover candidates must have unique adapter identifiers".into());
        }
        if !(1..=9999).contains(&candidate.priority) {
            return Err("candidate priorities must be between 1 and 9999".into());
        }
    }
    Ok(())
}

fn validate_remote_host(name: &str, endpoint: &str, fingerprint: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().count() > 80 {
        return Err("remote display name must contain 1–80 characters".into());
    }
    let endpoint = endpoint.trim();
    if !endpoint.starts_with("https://") || endpoint.len() > 255 || endpoint.contains('@') {
        return Err("remote endpoint must be an HTTPS URL without embedded credentials".into());
    }
    let normalized = normalize_fingerprint(fingerprint);
    if normalized.len() != 64 || !normalized.chars().all(|value| value.is_ascii_hexdigit()) {
        return Err(
            "certificate fingerprint must contain exactly 64 hexadecimal SHA-256 characters".into(),
        );
    }
    Ok(())
}

fn normalize_fingerprint(value: &str) -> String {
    value
        .chars()
        .filter(|value| *value != ':' && !value.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}

fn validate_profile_text(name: &str, description: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().count() > 80 {
        return Err("profile name must contain 1–80 characters".into());
    }
    if description.chars().count() > 500 {
        return Err("profile description cannot exceed 500 characters".into());
    }
    Ok(())
}
fn friendly_db_error(error: rusqlite::Error, fallback: &str) -> String {
    if error.to_string().contains("UNIQUE constraint") {
        fallback.into()
    } else {
        error.to_string()
    }
}
fn clone_export(value: &ProfileExport) -> ProfileExport {
    ProfileExport {
        schema_version: value.schema_version,
        name: value.name.clone(),
        description: value.description.clone(),
        adapters: value
            .adapters
            .iter()
            .map(|item| ProfileAdapterExport {
                adapter_match: item.adapter_match.clone(),
                address_mode: item.address_mode.clone(),
                address_config: item.address_config.clone(),
                route_role: item.route_role.clone(),
                priority: item.priority,
            })
            .collect(),
    }
}

fn parse_stored_static(value: &Value) -> Result<AddressConfiguration, String> {
    let address = value
        .get("ipv4Addresses")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .ok_or("stored static profile has no IPv4 address")?;
    let address = address
        .get("address")
        .and_then(Value::as_str)
        .ok_or("stored static IPv4 address is invalid")?
        .parse()
        .map_err(|_| "stored static IPv4 address is invalid")?;
    let prefix_length = address_prefix(value)?;
    let gateway = value
        .get("gateways")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .map(str::parse)
        .transpose()
        .map_err(|_| "stored gateway is invalid")?;
    let dns_servers = value
        .get("dnsServers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "stored DNS address is invalid")?;
    let interface_metric = value
        .get("interfaceMetric")
        .and_then(Value::as_u64)
        .unwrap_or(25)
        .clamp(1, 9999) as u32;
    let additional_addresses = value
        .get("ipv4Addresses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .skip(1)
        .filter_map(|entry| {
            Some(network_protocol::Ipv4AddressConfiguration {
                address: entry.get("address")?.as_str()?.parse().ok()?,
                prefix_length: u8::try_from(entry.get("prefixLength")?.as_u64()?).ok()?,
            })
        })
        .collect();
    Ok(AddressConfiguration::Static {
        address,
        prefix_length,
        additional_addresses,
        gateway,
        dns_servers,
        interface_metric,
    })
}

fn address_prefix(value: &Value) -> Result<u8, String> {
    value
        .get("ipv4Addresses")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(|entry| entry.get("prefixLength"))
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|value| *value <= 32)
        .ok_or_else(|| "stored IPv4 prefix is invalid".into())
}
