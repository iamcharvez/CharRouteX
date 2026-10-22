import { ArrowDownToLine, ArrowUpFromLine, Cable, Globe2, Router, Wifi } from "lucide-react";
import { formatBitsPerSecond, formatBytes, formatThroughput } from "../lib/utils";
import type { NetworkAdapter } from "../types/network";
import { Badge } from "./ui/badge";
import { Card } from "./ui/card";

export function AdapterCard({ adapter }: { adapter: NetworkAdapter }) {
  const isUp = adapter.status === "up";
  const Icon = adapter.kind === "wifi" ? Wifi : Cable;
  const addresses = [...adapter.ipv4Addresses, ...adapter.ipv6Addresses].map((item) => `${item.address}/${item.prefixLength}`);
  const health = adapter.connectivity;
  const healthTone = health?.state === "healthy"
    ? "border-emerald-400/20 bg-emerald-400/[0.07] text-emerald-300"
    : health?.state === "degraded"
      ? "border-amber-400/20 bg-amber-400/[0.07] text-amber-300"
      : "border-slate-400/15 bg-slate-400/[0.05] text-slate-500";

  return (
    <Card className="group relative overflow-hidden p-5 transition duration-300 hover:-translate-y-0.5 hover:border-cyan-300/20">
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-cyan-300/30 to-transparent opacity-0 transition group-hover:opacity-100" />
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <div className="grid h-11 w-11 shrink-0 place-items-center rounded-xl border border-white/[0.08] bg-white/[0.035] text-slate-300">
            <Icon size={21} />
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold text-slate-100">{adapter.name}</h3>
            <p className="mt-1 truncate text-[11px] text-slate-500">{adapter.description}</p>
          </div>
        </div>
        <Badge className={isUp ? "border-emerald-400/20 bg-emerald-400/[0.07] text-emerald-300" : "border-slate-400/15 bg-slate-400/[0.05] text-slate-500"}>
          <span className={`h-1.5 w-1.5 rounded-full ${isUp ? "bg-emerald-400" : "bg-slate-600"}`} />
          {adapter.status}
        </Badge>
      </div>

      <div className="mt-4 flex items-center justify-between gap-3 rounded-lg border border-white/[0.05] bg-black/10 px-3 py-2">
        <span className="text-[10px] uppercase tracking-[0.11em] text-slate-600">Connectivity</span>
        <Badge className={healthTone}>{health?.state.replace(/([A-Z])/g, " $1") ?? "checking"}</Badge>
      </div>

      <div className="my-5 grid grid-cols-2 gap-2">
        <InfoCell label={`IP addresses (${addresses.length})`} value={addresses.join(", ") || "Not assigned"} />
        <InfoCell label="Link speed" value={formatBitsPerSecond(Math.max(adapter.receiveLinkSpeedBps, adapter.transmitLinkSpeedBps))} />
        <InfoCell label="Gateway" value={adapter.gateways.join(", ") || "Local only"} icon={<Router size={12} />} />
        <InfoCell label="Metric" value={String(adapter.ipv4Metric || adapter.ipv6Metric || "Auto")} icon={<Globe2 size={12} />} />
        <InfoCell label="DNS servers" value={adapter.dnsServers.join(", ") || "Not reported"} />
        <InfoCell label="Address mode" value={adapter.dhcpEnabled ? "DHCP / automatic" : "Static / manual"} />
        <InfoCell label="MAC address" value={adapter.macAddress ?? "Not reported"} />
        <InfoCell label="Hardware" value={adapter.kind} />
      </div>

      <div className="flex items-center justify-between border-t border-white/[0.06] pt-4 text-[11px] text-slate-500">
        <span className="flex items-center gap-1.5" title={`RX · ${formatBytes(adapter.bytesReceived)} received total`}><ArrowDownToLine size={13} className="text-cyan-400" /> RX {adapter.downloadBytesPerSecond === null ? "Sampling" : formatThroughput(adapter.downloadBytesPerSecond)}</span>
        <span className="flex items-center gap-1.5" title={`TX · ${formatBytes(adapter.bytesSent)} transmitted total`}><ArrowUpFromLine size={13} className="text-blue-400" /> TX {adapter.uploadBytesPerSecond === null ? "Sampling" : formatThroughput(adapter.uploadBytesPerSecond)}</span>
        <span className="rounded-md bg-white/[0.035] px-2 py-1 font-mono">IF {adapter.interfaceIndex}</span>
      </div>
    </Card>
  );
}

function InfoCell({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div className="min-w-0 rounded-lg border border-white/[0.05] bg-black/10 px-3 py-2.5">
      <div className="mb-1 flex items-center gap-1 text-[10px] uppercase tracking-[0.11em] text-slate-600">{icon}{label}</div>
      <div className="truncate font-mono text-[11px] font-medium text-slate-300" title={value}>{value}</div>
    </div>
  );
}

