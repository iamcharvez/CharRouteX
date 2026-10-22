import { CheckCircle2, History, Route, ShieldCheck, Waypoints } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "../components/ui/badge";
import { Card } from "../components/ui/card";
import { Button } from "../components/ui/button";
import { applyAddressConfiguration, confirmOperation, getOperationStatus, restoreOperation, type OperationStatus } from "../services/network";
import { useRuntimeStore } from "../stores/app-store";
import { PageHeading } from "./dashboard";

export function RoutingPage() {
  const snapshot = useRuntimeStore((state) => state.snapshot);
  const routes = snapshot?.routes ?? [];
  const adapters = new Map((snapshot?.adapters ?? []).map((adapter) => [adapter.interfaceIndex, adapter.name]));
  const defaults = routes.filter((route) => route.isDefault);
  const [metrics, setMetrics] = useState<Record<number, number>>({});
  const [operation, setOperation] = useState<OperationStatus | null>(null);
  const preferredUnhealthy = defaults.find((route) => route.isPreferredDefault && !snapshot?.adapters.find((adapter) => adapter.interfaceIndex === route.interfaceIndex)?.connectivity?.internetReachable);
  const healthyAlternative = defaults.find((route) => snapshot?.adapters.find((adapter) => adapter.interfaceIndex === route.interfaceIndex)?.connectivity?.internetReachable);
  useEffect(() => {
    if (!operation || ["confirmed", "rolledBack", "failed"].includes(operation.state)) return;
    const timer = window.setInterval(() => void getOperationStatus(operation.operationId).then(setOperation).catch(() => undefined), 500);
    return () => window.clearInterval(timer);
  }, [operation]);

  async function applyMetric(interfaceIndex: number, current: number) {
    const metric = metrics[interfaceIndex] ?? current;
    if (!window.confirm(`Set interface ${interfaceIndex} metric to ${metric}? The previous metric will be restored unless you confirm within 60 seconds.`)) return;
    setOperation(await applyAddressConfiguration(interfaceIndex, { mode: "metricOnly", interfaceMetric: metric }));
  }

  return <section className="page-enter space-y-6">
    <PageHeading eyebrow="Native route inventory" title="Smart routing" description="Inspect Windows routes, correlate defaults with verified connectivity, and safely adjust interface preference without removing local subnet routes." />
    {preferredUnhealthy && healthyAlternative && preferredUnhealthy.interfaceIndex !== healthyAlternative.interfaceIndex && <div className="rounded-xl border border-amber-400/20 bg-amber-400/[0.06] p-4 text-sm text-amber-100"><div className="font-semibold">Potentially unsuitable preferred route detected</div><p className="mt-1 text-xs text-amber-200/60">Windows currently prefers {adapters.get(preferredUnhealthy.interfaceIndex)}, but {adapters.get(healthyAlternative.interfaceIndex)} has verified internet connectivity. Lower the healthy adapter metric to prefer it.</p></div>}
    {operation && <Card className="flex items-center gap-4 p-4"><ShieldCheck className="text-amber-300" /><div className="flex-1"><div className="text-xs font-semibold text-slate-300">{operation.state.replace(/([A-Z])/g, " $1")}</div><div className="mt-1 text-[11px] text-slate-500">{operation.message}</div></div>{operation.state === "applied" && <Button onClick={() => void confirmOperation(operation.operationId).then(() => getOperationStatus(operation.operationId)).then(setOperation)}><CheckCircle2 size={14} /> Keep metric</Button>}{operation.state === "confirmed" && <Button onClick={() => void restoreOperation(operation.operationId).then(setOperation)}><History size={14} /> Restore previous</Button>}</Card>}
    <div className="grid gap-4 lg:grid-cols-2">
      {defaults.map((route) => <Card key={`${route.destination}-${route.interfaceIndex}-${route.nextHop}`} className="p-5">
        <div className="flex items-start justify-between gap-4"><div className="flex items-center gap-3"><span className="grid h-10 w-10 place-items-center rounded-xl bg-blue-400/10 text-blue-300"><Waypoints size={19} /></span><div><h2 className="text-sm font-semibold text-slate-100">{adapters.get(route.interfaceIndex) ?? `Interface ${route.interfaceIndex}`}</h2><p className="mt-1 font-mono text-[11px] text-slate-500">via {route.nextHop}</p></div></div>{route.isPreferredDefault && <Badge className="border-emerald-400/20 bg-emerald-400/[0.07] text-emerald-300"><CheckCircle2 size={11} /> Preferred</Badge>}</div>
        <div className="mt-5 grid grid-cols-3 gap-2"><Metric label="Route metric" value={route.routeMetric} /><Metric label="Interface" value={route.interfaceMetric} /><Metric label="Effective" value={route.effectiveMetric} /></div><div className="mt-3 flex gap-2"><input className="input" type="number" min={1} max={9999} value={metrics[route.interfaceIndex] ?? route.interfaceMetric} onChange={(event) => setMetrics((values) => ({ ...values, [route.interfaceIndex]: Number(event.target.value) }))} /><Button onClick={() => void applyMetric(route.interfaceIndex, route.interfaceMetric)}>Set priority</Button></div>
      </Card>)}
      {defaults.length === 0 && <Card className="col-span-full grid min-h-40 place-items-center p-6 text-center text-sm text-slate-500"><div><Route className="mx-auto mb-3 text-slate-600" />No default routes reported by Windows.</div></Card>}
    </div>

    <Card className="overflow-hidden"><div className="flex items-center justify-between border-b border-white/[0.06] px-5 py-4"><div><h2 className="text-sm font-semibold text-slate-100">Windows routing table</h2><p className="mt-1 text-[11px] text-slate-500">{routes.length} IPv4 and IPv6 routes</p></div><Badge className="border-white/[0.08] bg-white/[0.03] text-slate-400">Read only</Badge></div><div className="max-h-[460px] overflow-auto"><table className="w-full min-w-[800px] text-left text-xs"><thead className="sticky top-0 bg-[#0d1b2d] text-[10px] uppercase tracking-wider text-slate-600"><tr><th className="px-5 py-3">Destination</th><th className="px-4 py-3">Next hop</th><th className="px-4 py-3">Interface</th><th className="px-4 py-3 text-right">Metric</th><th className="px-5 py-3">Role</th></tr></thead><tbody className="divide-y divide-white/[0.045]">{routes.map((route, index) => <tr key={`${route.destination}-${route.prefixLength}-${route.interfaceIndex}-${route.nextHop}-${index}`} className="text-slate-400 hover:bg-white/[0.02]"><td className="px-5 py-3 font-mono text-slate-300">{route.destination}/{route.prefixLength}</td><td className="px-4 py-3 font-mono">{route.nextHop}</td><td className="px-4 py-3">{adapters.get(route.interfaceIndex) ?? route.interfaceIndex}</td><td className="px-4 py-3 text-right font-mono">{route.effectiveMetric}</td><td className="px-5 py-3">{route.isPreferredDefault ? <span className="text-emerald-300">Preferred default</span> : route.isDefault ? <span className="text-blue-300">Default</span> : "Local / specific"}</td></tr>)}</tbody></table></div></Card>
  </section>;
}

function Metric({ label, value }: { label: string; value: number }) {
  return <div className="rounded-lg border border-white/[0.05] bg-black/10 p-3"><div className="text-[9px] uppercase tracking-wider text-slate-600">{label}</div><div className="mt-1 font-mono text-sm font-semibold text-slate-300">{value}</div></div>;
}

