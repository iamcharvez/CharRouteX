import { AlertTriangle, CheckCircle2, RadioTower, Save, ShieldCheck, Wifi } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { activateFailoverCandidate, getFailoverConfiguration, getFailoverStatus, saveFailoverConfiguration, type FailoverConfiguration, type FailoverStatus } from "../services/management";
import { confirmOperation, getOperationStatus, type OperationStatus } from "../services/network";
import { useRuntimeStore } from "../stores/app-store";
import { PageHeading } from "./dashboard";

const defaults: FailoverConfiguration = { automatic: false, policy: { failureThreshold: 3, recoveryThreshold: 3, cooldownSeconds: 30 }, candidates: [] };

export function FailoverPage() {
  const adapters = useRuntimeStore((state) => state.snapshot?.adapters ?? []);
  const eligible = useMemo(() => adapters.filter((adapter) => adapter.status === "up" && adapter.gateways.length > 0), [adapters]);
  const [configuration, setConfiguration] = useState(defaults);
  const [status, setStatus] = useState<FailoverStatus | null>(null);
  const [operation, setOperation] = useState<OperationStatus | null>(null);
  const [operationTarget, setOperationTarget] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const switching = useRef(false);
  useEffect(() => { void getFailoverConfiguration().then(setConfiguration).catch((error) => setMessage(String(error))); }, []);
  useEffect(() => setConfiguration((value) => ({ ...value, candidates: eligible.map((adapter, index) => value.candidates.find((candidate) => candidate.adapterId === adapter.id) ?? { adapterId: adapter.id, priority: (index + 1) * 10, enabled: true }) })), [eligible]);
  const refreshStatus = useCallback(() => getFailoverStatus().then(setStatus).catch((error) => setMessage(String(error))), []);
  useEffect(() => { void refreshStatus(); const timer = window.setInterval(() => void refreshStatus(), 5000); return () => window.clearInterval(timer); }, [refreshStatus]);
  async function activate(adapterId: string, automatic = false) {
    if (switching.current) return;
    const adapter = adapters.find((item) => item.id === adapterId); const candidate = configuration.candidates.find((item) => item.adapterId === adapterId);
    if (!adapter || !candidate) return;
    if (!automatic && !window.confirm(`Prefer ${adapter.name} for internet traffic? Windows may interrupt existing TCP sessions. The previous metric is restored unless the result is verified and confirmed within 60 seconds.`)) return;
    switching.current = true; setOperationTarget(adapterId);
    try { setOperation(await activateFailoverCandidate(adapter.interfaceIndex, Math.max(1, Math.min(9999, candidate.priority)))); setMessage(automatic ? `Automatic failover requested for ${adapter.name}. Approve the Windows authorization prompt.` : `Route change requested for ${adapter.name}.`); }
    catch (error) { switching.current = false; setMessage(String(error)); }
  }
  useEffect(() => { if (configuration.automatic && status?.decision && !operation) void activate(status.decision.to, true); }, [status, configuration.automatic, operation]);
  useEffect(() => {
    if (!operation || ["confirmed", "rolledBack", "failed"].includes(operation.state)) { if (operation) switching.current = false; return; }
    const timer = window.setInterval(async () => {
      try {
        const next = await getOperationStatus(operation.operationId); setOperation(next);
        const targetHealthy = adapters.find((adapter) => adapter.id === operationTarget)?.connectivity?.internetReachable;
        if (next.state === "applied" && targetHealthy) { await confirmOperation(next.operationId); setOperation(await getOperationStatus(next.operationId)); setMessage("The selected uplink passed connectivity verification and the route preference was kept."); }
      } catch { /* UAC may still be pending. */ }
    }, 750);
    return () => window.clearInterval(timer);
  }, [operation, operationTarget, adapters]);
  async function save() { try { await saveFailoverConfiguration(configuration); setMessage("Failover policy saved."); await refreshStatus(); } catch (error) { setMessage(String(error)); } }
  function patchCandidate(adapterId: string, patch: Partial<{ enabled: boolean; priority: number }>) { setConfiguration((value) => ({ ...value, candidates: value.candidates.map((item) => item.adapterId === adapterId ? { ...item, ...patch } : item) })); }
  return <section className="page-enter space-y-6"><PageHeading eyebrow="Continuity policy" title="Internet failover" description="Rank genuine internet-capable interfaces, require repeated health evidence, and safely change Windows route preference without disabling LAN-only adapters." action={<Button onClick={() => void save()}><Save size={14} /> Save policy</Button>} />
    <div className="rounded-xl border border-amber-400/20 bg-amber-400/[0.06] p-4 text-xs text-amber-100"><div className="flex items-center gap-2 font-semibold"><AlertTriangle size={15} /> Upstream changes can disconnect existing sessions</div><p className="mt-1 text-amber-200/60">Local subnet routes remain intact. Automatic mode is opt-in and Windows authorization is required for elevated metric changes.</p></div>
    <div className="grid gap-4 lg:grid-cols-3"><PolicyNumber label="Failure threshold" value={configuration.policy.failureThreshold} min={1} max={10} onChange={(failureThreshold) => setConfiguration((value) => ({ ...value, policy: { ...value.policy, failureThreshold } }))} /><PolicyNumber label="Recovery threshold" value={configuration.policy.recoveryThreshold} min={1} max={10} onChange={(recoveryThreshold) => setConfiguration((value) => ({ ...value, policy: { ...value.policy, recoveryThreshold } }))} /><PolicyNumber label="Cooldown seconds" value={configuration.policy.cooldownSeconds} min={5} max={3600} onChange={(cooldownSeconds) => setConfiguration((value) => ({ ...value, policy: { ...value.policy, cooldownSeconds } }))} /></div>
    <Card className="flex items-center gap-4 p-5"><span className="grid h-10 w-10 place-items-center rounded-xl bg-cyan-400/10 text-cyan-300"><RadioTower size={19} /></span><div className="flex-1"><h2 className="text-sm font-semibold text-slate-100">Automatic route selection</h2><p className="mt-1 text-xs text-slate-500">Switch only after both thresholds and the cooldown policy are satisfied.</p></div><button onClick={() => { const automatic = !configuration.automatic; if (!automatic || window.confirm("Enable automatic failover? CharRouteX may request administrator authorization when a verified uplink fails.")) setConfiguration((value) => ({ ...value, automatic })); }} className={`rounded-full border px-4 py-2 text-xs font-semibold ${configuration.automatic ? "border-emerald-400/25 bg-emerald-400/10 text-emerald-300" : "border-white/[0.08] text-slate-500"}`}>{configuration.automatic ? "Enabled" : "Manual"}</button></Card>
    {operation && <Card className="flex items-center gap-3 p-4"><ShieldCheck className="text-amber-300" size={19} /><div className="flex-1"><div className="text-xs font-semibold text-slate-300">{operation.state.replace(/([A-Z])/g, " $1")}</div><div className="mt-1 text-[11px] text-slate-500">{operation.message}</div></div>{operation.state === "applied" && <span className="text-xs text-amber-300">Verifying connectivity…</span>}</Card>}
    <div className="grid gap-4 xl:grid-cols-2">{eligible.map((adapter) => { const candidate = configuration.candidates.find((item) => item.adapterId === adapter.id); const active = status?.activeAdapterId === adapter.id; return <Card key={adapter.id} className="p-5"><div className="flex items-start gap-3"><span className="grid h-10 w-10 place-items-center rounded-xl bg-blue-400/10 text-blue-300"><Wifi size={18} /></span><div className="min-w-0 flex-1"><h3 className="truncate text-sm font-semibold text-slate-100">{adapter.name}</h3><p className="mt-1 truncate text-[11px] text-slate-500">{adapter.description}</p></div>{active && <Badge className="border-emerald-400/20 bg-emerald-400/[0.07] text-emerald-300"><CheckCircle2 size={11} /> Active</Badge>}</div><div className="mt-4 flex items-end gap-3"><label className="flex-1 text-[10px] uppercase tracking-wider text-slate-500">Priority<input className="input mt-2" type="number" min={1} max={9999} value={candidate?.priority ?? 10} onChange={(event) => patchCandidate(adapter.id, { priority: Number(event.target.value) })} /></label><button onClick={() => patchCandidate(adapter.id, { enabled: !candidate?.enabled })} className={`h-[42px] rounded-lg border px-3 text-xs ${candidate?.enabled ? "border-cyan-400/25 bg-cyan-400/10 text-cyan-200" : "border-white/[0.08] text-slate-600"}`}>{candidate?.enabled ? "Eligible" : "Excluded"}</button><Button disabled={active || !candidate?.enabled || switching.current} onClick={() => void activate(adapter.id)}>Make active</Button></div></Card>; })}{eligible.length === 0 && <Card className="col-span-full grid min-h-40 place-items-center text-sm text-slate-600">No linked adapters with a gateway are currently available.</Card>}</div>{message && <div className="rounded-lg border border-white/[0.07] bg-white/[0.03] px-4 py-3 text-xs text-slate-400">{message}</div>}</section>;
}

function PolicyNumber({ label, value, min, max, onChange }: { label: string; value: number; min: number; max: number; onChange: (value: number) => void }) { return <Card className="p-4"><label className="text-[10px] font-semibold uppercase tracking-wider text-slate-500">{label}<input className="input mt-2" type="number" min={min} max={max} value={value} onChange={(event) => onChange(Number(event.target.value))} /></label></Card>; }
