import { Database, Github, Network, Route, ShieldCheck } from "lucide-react";
import { Card } from "../components/ui/card";
import { PageHeading } from "./dashboard";

export function AboutPage() {
  return <section className="page-enter space-y-6">
    <PageHeading eyebrow="CharRouteX 1.1.0" title="Your Network. Your Control." description="A safety-first Windows network interface, IP configuration, intelligent routing, and connectivity management application." />
    <Card className="max-w-4xl p-7">
      <div className="flex items-center gap-5"><span className="grid h-16 w-16 place-items-center rounded-2xl border border-cyan-300/25 bg-cyan-400/10 text-cyan-300"><Network size={30} /></span><div><h2 className="text-lg font-bold text-slate-100">CharRoute<span className="text-cyan-300">X</span></h2><p className="mt-1 text-xs text-slate-500">Windows 10/11 · Tauri 2 · Rust · React</p></div></div>
      <div className="mt-7 grid gap-3 sm:grid-cols-2"><Fact icon={ShieldCheck} title="Protected changes" detail="Typed UAC helper, backup, confirmation, rollback" /><Fact icon={Route} title="Intelligent routing" detail="Native routes, health thresholds, failover policy" /><Fact icon={Database} title="Local persistence" detail="SQLite profiles, health history, audit trail" /><Fact icon={Github} title="Open architecture" detail="Modular Rust workspace; no fixed adapter limit" /></div>
      <p className="mt-7 border-t border-white/[0.06] pt-5 text-xs leading-6 text-slate-500">Ordinary failover changes route preference; it does not combine bandwidth. True bonding requires a separate aggregation tunnel and endpoint. Remote hosts are accepted only after explicit enrollment and certificate-pinned TLS verification.</p>
    </Card>
  </section>;
}

function Fact({ icon: Icon, title, detail }: { icon: typeof Network; title: string; detail: string }) {
  return <div className="flex items-center gap-3 rounded-xl border border-white/[0.06] bg-white/[0.02] p-4"><Icon className="text-emerald-300" size={19} /><div><div className="text-xs font-semibold text-slate-300">{title}</div><div className="mt-1 text-[11px] text-slate-600">{detail}</div></div></div>;
}
