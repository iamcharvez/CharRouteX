import { Activity, Gauge, Moon, RefreshCw, Sun, XCircle } from "lucide-react";
import { Card } from "../components/ui/card";
import { setMonitoringPaused } from "../services/network";
import { useRuntimeStore, useSettingsStore } from "../stores/app-store";
import { PageHeading } from "./dashboard";

export function SettingsPage() {
  const { theme, setTheme, refreshIntervalMs, setRefreshIntervalMs, healthProbeIntervalSeconds, setHealthProbeIntervalSeconds, closeToTray, setCloseToTray } = useSettingsStore();
  const { monitoringPaused, setMonitoringPaused: setPausedState } = useRuntimeStore();
  async function toggleMonitoring() { const paused = !monitoringPaused; await setMonitoringPaused(paused); setPausedState(paused); }
  return <section className="page-enter space-y-6">
    <PageHeading eyebrow="Application" title="Settings" description="Adjust background behavior, monitoring frequency, traffic responsiveness, and presentation." />
    <div className="grid max-w-5xl gap-4 md:grid-cols-2">
      <Card className="p-5"><SettingHeading icon={theme === "dark" ? Moon : Sun} title="Appearance" description="Choose the application color theme." /><div className="mt-5 grid grid-cols-2 gap-2">{(["dark", "light"] as const).map((value) => <Choice key={value} selected={theme === value} onClick={() => setTheme(value)}>{value}</Choice>)}</div></Card>
      <Card className="p-5"><SettingHeading icon={RefreshCw} title="Traffic refresh speed" description="How frequently byte counters are sampled and converted to RX/TX speed." /><select value={refreshIntervalMs} onChange={(event) => setRefreshIntervalMs(Number(event.target.value))} className="input mt-5"><option value={500}>Every 0.5 seconds (fast)</option><option value={1000}>Every 1 second (recommended)</option><option value={2000}>Every 2 seconds</option><option value={5000}>Every 5 seconds</option><option value={10000}>Every 10 seconds</option></select><p className="mt-3 text-[10px] leading-5 text-slate-600">This changes measurement responsiveness, not the physical internet speed.</p></Card>
      <Card className="p-5"><SettingHeading icon={Gauge} title="Connectivity verification" description="How often gateway, DNS, TCP, and HTTPS health are rechecked." /><select value={healthProbeIntervalSeconds} onChange={(event) => setHealthProbeIntervalSeconds(Number(event.target.value))} className="input mt-5"><option value={3}>Every 3 seconds (aggressive)</option><option value={5}>Every 5 seconds</option><option value={10}>Every 10 seconds (recommended)</option><option value={30}>Every 30 seconds</option><option value={60}>Every minute</option></select></Card>
      <Card className="p-5"><SettingHeading icon={XCircle} title="When the window is closed" description="Keep monitoring in the Windows notification area or exit completely." /><div className="mt-5 grid grid-cols-2 gap-2"><Choice selected={closeToTray} onClick={() => setCloseToTray(true)}>Keep in tray</Choice><Choice selected={!closeToTray} onClick={() => setCloseToTray(false)}>Exit application</Choice></div><p className="mt-3 text-[10px] leading-5 text-slate-600">When kept in the tray, right-click the CharRouteX icon to open, refresh, pause, or exit.</p></Card>
      <Card className="p-5 md:col-span-2"><div className="flex items-center gap-4"><SettingHeading icon={Activity} title="Background monitor" description="Pause sampling without exiting CharRouteX. The tray menu provides the same control." /><button onClick={() => void toggleMonitoring()} className={`ml-auto rounded-full border px-4 py-2 text-xs font-semibold ${monitoringPaused ? "border-amber-400/25 bg-amber-400/10 text-amber-300" : "border-emerald-400/25 bg-emerald-400/10 text-emerald-300"}`}>{monitoringPaused ? "Resume monitoring" : "Pause monitoring"}</button></div></Card>
    </div>
  </section>;
}

function SettingHeading({ icon: Icon, title, description }: { icon: typeof Moon; title: string; description: string }) { return <div className="flex items-center gap-3"><span className="grid h-10 w-10 shrink-0 place-items-center rounded-lg bg-cyan-400/10 text-cyan-300"><Icon size={18} /></span><div><h2 className="text-sm font-semibold text-slate-100">{title}</h2><p className="mt-1 text-xs text-slate-500">{description}</p></div></div>; }
function Choice({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: React.ReactNode }) { return <button onClick={onClick} className={`rounded-lg border px-3 py-2 text-xs font-medium capitalize transition ${selected ? "border-cyan-400/30 bg-cyan-400/10 text-cyan-200" : "border-white/[0.07] bg-white/[0.02] text-slate-500 hover:text-slate-300"}`}>{children}</button>; }
