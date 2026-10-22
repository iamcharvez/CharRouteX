import { CheckCircle2, CircleX, Clock3, Gauge, History, Network } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "../components/ui/badge";
import { Card } from "../components/ui/card";
import { useRuntimeStore } from "../stores/app-store";
import { PageHeading } from "./dashboard";
import { getHealthHistory, type HealthSample } from "../services/management";
import { formatThroughput } from "../lib/utils";

export function DiagnosticsPage() {
  const adapters = useRuntimeStore((state) => state.snapshot?.adapters ?? []);
  const [selected, setSelected] = useState("");
  const [history, setHistory] = useState<HealthSample[]>([]);
  useEffect(() => { if (!selected && adapters[0]) setSelected(adapters[0].id); }, [adapters, selected]);
  useEffect(() => { if (selected) void getHealthHistory(selected).then(setHistory); }, [selected, adapters]);
  return <section className="page-enter space-y-6">
    <PageHeading eyebrow="Interface-aware probes" title="Network diagnostics" description="Each linked interface is checked independently through its assigned source address using gateway neighbor resolution, configured DNS, two bounded TCP checks, and a certificate-validated HTTPS request." />
    <div className="grid gap-4 xl:grid-cols-2">{adapters.filter((adapter) => adapter.status === "up").map((adapter) => {
      const health = adapter.connectivity;
      return <Card key={adapter.id} className="p-5"><div className="flex items-start justify-between"><div className="flex items-center gap-3"><span className="grid h-10 w-10 place-items-center rounded-xl bg-cyan-400/10 text-cyan-300"><Network size={19} /></span><div><h2 className="text-sm font-semibold text-slate-100">{adapter.name}</h2><p className="mt-1 text-[11px] text-slate-500">Source {adapter.ipv4Addresses[0]?.address ?? adapter.ipv6Addresses[0]?.address ?? "unassigned"}</p></div></div><Badge className={health?.internetReachable ? "border-emerald-400/20 bg-emerald-400/[0.07] text-emerald-300" : "border-amber-400/20 bg-amber-400/[0.07] text-amber-300"}>{health?.state.replace(/([A-Z])/g, " $1") ?? "checking"}</Badge></div>
        <div className="mt-5 grid grid-cols-2 gap-2"><Stat icon={Clock3} label="Mean latency" value={health?.latencyMs == null ? "—" : `${health.latencyMs.toFixed(0)} ms`} /><Stat icon={Gauge} label="Probe loss" value={health?.packetLoss == null ? "—" : `${(health.packetLoss * 100).toFixed(0)}%`} /></div>
        <div className="mt-4 space-y-2">{health?.probes.map((probe) => <div key={probe.name} className="flex items-center gap-3 rounded-lg border border-white/[0.05] bg-black/10 px-3 py-2.5">{probe.success ? <CheckCircle2 size={15} className="shrink-0 text-emerald-400" /> : <CircleX size={15} className="shrink-0 text-rose-400" />}<div className="min-w-0 flex-1"><div className="text-xs font-medium text-slate-300">{probe.name}</div><div className="mt-0.5 truncate text-[10px] text-slate-600" title={probe.detail}>{probe.detail}</div></div><span className="font-mono text-[10px] text-slate-500">{probe.latencyMs == null ? "—" : `${probe.latencyMs.toFixed(0)} ms`}</span></div>)}</div>
      </Card>;
    })}</div>
    <Card className="p-5"><div className="flex items-center justify-between gap-4"><div className="flex items-center gap-3"><span className="grid h-10 w-10 place-items-center rounded-xl bg-blue-400/10 text-blue-300"><History size={18} /></span><div><h2 className="text-sm font-semibold text-slate-100">24-hour health history</h2><p className="mt-1 text-[11px] text-slate-500">SQLite samples are retained locally for 30 days.</p></div></div><select className="input max-w-xs" value={selected} onChange={(event) => setSelected(event.target.value)}>{adapters.map((adapter) => <option key={adapter.id} value={adapter.id}>{adapter.name}</option>)}</select></div><HistoryChart samples={history} /></Card>
  </section>;
}

function HistoryChart({ samples }: { samples: HealthSample[] }) {
  if (samples.length === 0) return <div className="mt-5 grid h-32 place-items-center rounded-lg border border-dashed border-white/[0.08] text-xs text-slate-600">History appears after the monitor records its first sample.</div>;
  const latency = samples.map((item) => item.latencyMs ?? 0); const max = Math.max(1, ...latency); const width = 900; const height = 120;
  const points = latency.map((value, index) => `${samples.length === 1 ? 0 : index / (samples.length - 1) * width},${height - value / max * (height - 12)}`).join(" ");
  const latest = samples.at(-1)!; const healthy = samples.filter((item) => item.state === "healthy" || item.state === "degraded").length / samples.length * 100;
  return <div className="mt-5"><div className="grid grid-cols-3 gap-2"><HistoryMetric label="Samples" value={String(samples.length)} /><HistoryMetric label="Internet availability" value={`${healthy.toFixed(1)}%`} /><HistoryMetric label="Latest traffic" value={`${formatThroughput(latest.downloadBytesPerSecond ?? 0)} ↓ · ${formatThroughput(latest.uploadBytesPerSecond ?? 0)} ↑`} /></div><div className="mt-3 overflow-hidden rounded-lg border border-white/[0.05] bg-black/10 p-3"><svg viewBox={`0 0 ${width} ${height}`} className="h-32 w-full" preserveAspectRatio="none"><defs><linearGradient id="latency" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#22d3ee" stopOpacity=".35" /><stop offset="1" stopColor="#22d3ee" stopOpacity="0" /></linearGradient></defs><polyline points={`0,${height} ${points} ${width},${height}`} fill="url(#latency)" stroke="none" /><polyline points={points} fill="none" stroke="#22d3ee" strokeWidth="2" vectorEffect="non-scaling-stroke" /></svg><div className="mt-1 flex justify-between text-[9px] uppercase tracking-wider text-slate-600"><span>{new Date(samples[0]!.sampledAt).toLocaleTimeString()}</span><span>Latency · max {max.toFixed(0)} ms</span><span>{new Date(latest.sampledAt).toLocaleTimeString()}</span></div></div></div>;
}

function HistoryMetric({ label, value }: { label: string; value: string }) { return <div className="rounded-lg border border-white/[0.05] bg-white/[0.02] p-3"><div className="text-[9px] uppercase tracking-wider text-slate-600">{label}</div><div className="mt-1 truncate text-xs font-semibold text-slate-300">{value}</div></div>; }

function Stat({ icon: Icon, label, value }: { icon: typeof Clock3; label: string; value: string }) {
  return <div className="flex items-center gap-3 rounded-lg border border-white/[0.05] bg-white/[0.02] p-3"><Icon size={15} className="text-cyan-400" /><div><div className="text-[9px] uppercase tracking-wider text-slate-600">{label}</div><div className="mt-1 text-xs font-semibold text-slate-300">{value}</div></div></div>;
}

