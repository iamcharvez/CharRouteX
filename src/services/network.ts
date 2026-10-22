import { invoke } from "@tauri-apps/api/core";
import type { AdapterSnapshot, ValidationReport } from "../types/network";

export async function getNetworkAdapters(): Promise<AdapterSnapshot> {
  return invoke<AdapterSnapshot>("get_network_adapters");
}

export interface StaticIpv4Configuration {
  address: string;
  prefixLength: number;
  additionalAddresses: Array<{ address: string; prefixLength: number }>;
  gateway: string | null;
  dnsServers: string[];
  interfaceMetric: number;
}

export async function validateIpv4Configuration(interfaceIndex: number, config: StaticIpv4Configuration): Promise<ValidationReport> {
  return invoke<ValidationReport>("validate_ipv4_configuration", { interfaceIndex, config });
}

export type AddressConfiguration = { mode: "dhcp" } | { mode: "dhcpRenew" } | { mode: "dhcpRelease" } | { mode: "metricOnly"; interfaceMetric: number } | ({ mode: "static" } & StaticIpv4Configuration);
export interface OperationStatus {
  operationId: string;
  state: "pending" | "applied" | "confirmed" | "rollingBack" | "rolledBack" | "failed";
  message: string;
  rollbackDeadlineUnixMs: number | null;
}
export const applyAddressConfiguration = (interfaceIndex: number, configuration: AddressConfiguration) => invoke<OperationStatus>("apply_address_configuration", { interfaceIndex, configuration });
export const getOperationStatus = (operationId: string) => invoke<OperationStatus>("get_network_operation_status", { operationId });
export const confirmOperation = (operationId: string) => invoke<void>("confirm_network_operation", { operationId });
export const restoreOperation = (operationId: string) => invoke<OperationStatus>("restore_network_operation", { operationId });
export interface OperationVerification { safeToConfirm: boolean; checks: Array<{ name: string; passed: boolean; detail: string }> }
export const verifyOperation = (operationId: string) => invoke<OperationVerification>("verify_network_operation", { operationId });
export const setMonitoringPaused = (paused: boolean) => invoke<void>("set_monitoring_paused", { paused });
export const getMonitoringPaused = () => invoke<boolean>("get_monitoring_paused");
export const setHealthProbeInterval = (seconds: number) => invoke<void>("set_health_probe_interval", { seconds });
export const setCloseToTray = (enabled: boolean) => invoke<void>("set_close_to_tray", { enabled });

