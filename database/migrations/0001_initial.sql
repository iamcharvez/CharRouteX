PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE adapter_identities (
    id TEXT PRIMARY KEY NOT NULL,
    platform TEXT NOT NULL CHECK (platform IN ('windows', 'linux', 'macos')),
    system_id TEXT NOT NULL,
    interface_guid TEXT,
    mac_address TEXT,
    last_friendly_name TEXT NOT NULL,
    hardware_description TEXT,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    UNIQUE (platform, system_id)
) STRICT;

CREATE TABLE profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    schema_version INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (name)
) STRICT;

CREATE TABLE profile_adapters (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    adapter_match_json TEXT NOT NULL,
    address_mode TEXT NOT NULL CHECK (address_mode IN ('dhcp', 'static', 'unchanged')),
    address_config_json TEXT NOT NULL,
    route_role TEXT NOT NULL CHECK (route_role IN ('local_only', 'primary', 'backup', 'eligible', 'unchanged')),
    priority INTEGER,
    CHECK (priority IS NULL OR priority >= 0),
    UNIQUE (profile_id, id)
) STRICT;

CREATE TABLE health_samples (
    id INTEGER PRIMARY KEY,
    adapter_id TEXT NOT NULL REFERENCES adapter_identities(id) ON DELETE CASCADE,
    sampled_at TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('disconnected', 'local_only', 'gateway', 'healthy', 'degraded', 'unknown')),
    latency_ms REAL,
    packet_loss REAL CHECK (packet_loss IS NULL OR (packet_loss >= 0 AND packet_loss <= 1)),
    bytes_sent_delta INTEGER,
    bytes_received_delta INTEGER,
    details_json TEXT NOT NULL
) STRICT;

CREATE INDEX health_samples_adapter_time ON health_samples(adapter_id, sampled_at DESC);

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    occurred_at TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('debug', 'info', 'warning', 'error', 'security')),
    category TEXT NOT NULL,
    actor TEXT NOT NULL,
    host_id TEXT,
    operation_id TEXT,
    message TEXT NOT NULL,
    details_json TEXT NOT NULL
) STRICT;

CREATE INDEX audit_events_time ON audit_events(occurred_at DESC);
CREATE INDEX audit_events_operation ON audit_events(operation_id);

CREATE TABLE configuration_backups (
    id TEXT PRIMARY KEY NOT NULL,
    host_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    state_json TEXT NOT NULL,
    checksum TEXT NOT NULL
) STRICT;

CREATE TABLE network_operations (
    id TEXT PRIMARY KEY NOT NULL,
    host_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('planned', 'backed_up', 'applied', 'verifying', 'confirmed', 'rolling_back', 'rolled_back', 'failed')),
    requested_at TEXT NOT NULL,
    rollback_deadline TEXT,
    backup_id TEXT REFERENCES configuration_backups(id),
    plan_json TEXT NOT NULL,
    result_json TEXT
) STRICT;

CREATE UNIQUE INDEX one_active_operation_per_host ON network_operations(host_id)
WHERE state IN ('backed_up', 'applied', 'verifying', 'rolling_back');

CREATE TABLE remote_hosts (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    certificate_fingerprint TEXT NOT NULL,
    credential_reference TEXT NOT NULL,
    authorization_policy_json TEXT NOT NULL,
    enrolled_at TEXT NOT NULL,
    last_seen_at TEXT,
    revoked_at TEXT
) STRICT;

