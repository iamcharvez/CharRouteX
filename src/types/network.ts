export type AdapterKind =
  | "ethernet"
  | "wifi"
  | "loopback"
  | "tunnel"
  | "virtual"
  | "other";

export type LinkStatus = "up" | "down" | "dormant" | "unknown";
export type ConnectivityState = "disconnected" | "localOnly" | "gatewayReachable" | "healthy" | "degraded" | "unknown";

export interface ProbeResult {
  name: string;
  success: boolean;
  latencyMs: number | null;
  detail: string;
}

export interface ConnectivityHealth {
  state: ConnectivityState;
  physicalConnected: boolean;
  localNetworkReachable: boolean;
  gatewayReachable: boolean | null;
  dnsReachable: boolean;
  internetReachable: boolean;
  degraded: boolean;
  latencyMs: number | null;
  packetLoss: number | null;
  probes: ProbeResult[];
}

export interface IpAddress {
  address: string;
  prefixLength: number;
}

export interface NetworkAdapter {
  id: string;
  interfaceIndex: number;
  name: string;
  description: string;
  kind: AdapterKind;
  macAddress: string | null;
  status: LinkStatus;
  transmitLinkSpeedBps: number;
  receiveLinkSpeedBps: number;
  ipv4Addresses: IpAddress[];
  ipv6Addresses: IpAddress[];
  gateways: string[];
  dnsServers: string[];
  dhcpEnabled: boolean;
  ipv4Metric: number;
  ipv6Metric: number;
  bytesSent: number;
  bytesReceived: number;
  uploadBytesPerSecond: number | null;
  downloadBytesPerSecond: number | null;
  connectivity: ConnectivityHealth | null;
}

export interface RouteEntry {
  interfaceIndex: number;
  destination: string;
  prefixLength: number;
  nextHop: string;
  routeMetric: number;
  interfaceMetric: number;
  effectiveMetric: number;
  protocol: number;
  ageSeconds: number;
  isDefault: boolean;
  isPreferredDefault: boolean;
}

export interface AdapterSnapshot {
  collectedAt: string;
  adapters: NetworkAdapter[];
  routes: RouteEntry[];
}

export interface ValidationIssue {
  field: string;
  severity: "error" | "warning";
  code: string;
  message: string;
}

export interface ValidationReport {
  valid: boolean;
  issues: ValidationIssue[];
}

