import { invoke } from "@tauri-apps/api/core";
import type { OperationStatus } from "./network";

export interface FailoverCandidateConfiguration { adapterId: string; priority: number; enabled: boolean }
export interface FailoverConfiguration {
  automatic: boolean;
  policy: { failureThreshold: number; recoveryThreshold: number; cooldownSeconds: number };
  candidates: FailoverCandidateConfiguration[];
}
export interface FailoverStatus { activeAdapterId: string | null; decision: { from: string | null; to: string; reason: string } | null }
export const getFailoverConfiguration = () => invoke<FailoverConfiguration>("get_failover_configuration");
export const saveFailoverConfiguration = (configuration: FailoverConfiguration) => invoke<void>("save_failover_configuration", { configuration });
export const getFailoverStatus = () => invoke<FailoverStatus>("get_failover_status");
export const activateFailoverCandidate = (interfaceIndex: number, metric: number) => invoke<OperationStatus>("activate_failover_candidate", { interfaceIndex, metric });

export interface HealthSample { sampledAt: string; state: string; latencyMs: number | null; packetLoss: number | null; uploadBytesPerSecond: number | null; downloadBytesPerSecond: number | null }
export const getHealthHistory = (adapterId: string, hours = 24) => invoke<HealthSample[]>("get_health_history", { adapterId, hours });

export interface RemoteHost { id: string; displayName: string; endpoint: string; certificateFingerprint: string; enrolledAt: string; lastSeenAt: string | null; revoked: boolean }
export interface RemoteProbeResult { reachable: boolean; authenticated: boolean; latencyMs: number; message: string }
export const listRemoteHosts = () => invoke<RemoteHost[]>("list_remote_hosts");
export const enrollRemoteHost = (displayName: string, endpoint: string, certificateFingerprint: string) => invoke<string>("enroll_remote_host", { displayName, endpoint, certificateFingerprint });
export const revokeRemoteHost = (id: string) => invoke<void>("revoke_remote_host", { id });
export const probeRemoteHost = (endpoint: string, certificateFingerprint: string) => invoke<RemoteProbeResult>("probe_remote_host", { endpoint, certificateFingerprint });
