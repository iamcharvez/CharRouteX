import { Activity, Cable, CircleAlert, Gauge, RefreshCw, Route, Wifi } from "lucide-react";
import { AdapterCard } from "../components/adapter-card";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { useRuntimeStore } from "../stores/app-store";

export function Dashboard({ onRefresh }: { onRefresh: () => void }) {
  const { snapshot, loading, error } = useRuntimeStore();
  const adapters = snapshot?.adapters ?? [];
  const linked = adapters.filter((adapter) => adapter.status === "up");
  const addressed = linked.filter((adapter) => adapter.ipv4Addresses.length + adapter.ipv6Addresses.length > 0);
  const gatewayCount = linked.filter((adapter) => adapter.connectivity?.gatewayReachable === true).length;
  const internetCount = linked.filter((adapter) => adapter.connectivity?.internetReachable).length;

  return (
    <section className="page-enter space-y-6">
      <PageHeading
        eyebrow="Live overview"
        title="Network dashboard"
        description="Real-time Windows interface state, per-adapter connectivity health, route preference, and sampled traffic in bits per second."
        action={<Button onClick={onRefresh} disabled={loading}><RefreshCw size={15} className={loading ? "animate-spin" : ""} /> Refresh</Button>}
      />

      {error && (
        <div className="flex items-start gap-3 rounded-xl border border-amber-400/20 bg-amber-400/[0.06] p-4 text-sm text-amber-100">
          <CircleAlert size={18} className="mt-0.5 shrink-0 text-amber-300" />
          <div><div className="font-semibold">Adapter data is unavailable</div><div className="mt-1 text-xs text-amber-200/60">{error}</div></div>
        </div>
      )}

      <div className="grid gap-4 xl:grid-cols-4">
        <Summary icon={Cable} label="Detected adapters" value={adapters.length} detail={`${linked.length} linked`} color="cyan" />
        <Summary icon={Activity} label="Addressed links" value={addressed.length} detail="IPv4 or IPv6 assigned" color="emerald" />
        <Summary icon={Route} label="Reachable gateways" value={gatewayCount} detail="Verified on-link" color="blue" />
        <Summary icon={Gauge} label="Internet uplinks" value={internetCount} detail={snapshot ? `Updated ${new Date(snapshot.collectedAt).toLocaleTimeString()}` : "Waiting for first sample"} color="violet" />
      </div>

      <div className="flex items-end justify-between gap-4">
        <div>
          <h2 className="text-base font-semibold text-slate-100">Interfaces</h2>
          <p className="mt-1 text-xs text-slate-500">A dynamic inventory with no fixed adapter limit.</p>
        </div>
        <Badge className="border-white/[0.08] bg-white/[0.03] text-slate-400"><span className="h-1.5 w-1.5 rounded-full bg-cyan-400" /> Native Windows data</Badge>
      </div>

      {loading && !snapshot ? (
        <div className="grid gap-4 xl:grid-cols-2"><Skeleton /><Skeleton /></div>
      ) : adapters.length ? (
        <div className="grid gap-4 xl:grid-cols-2">{adapters.map((adapter) => <AdapterCard key={adapter.id} adapter={adapter} />)}</div>
      ) : !error ? (
        <Card className="grid min-h-56 place-items-center p-8 text-center"><div><Wifi size={28} className="mx-auto text-slate-600" /><p className="mt-3 text-sm text-slate-400">Windows reported no network interfaces.</p></div></Card>
      ) : null}
    </section>
  );
}

function PageHeading({ eyebrow, title, description, action }: { eyebrow: string; title: string; description: string; action?: React.ReactNode }) {
  return <div className="flex items-end justify-between gap-5"><div><p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-cyan-400/70">{eyebrow}</p><h1 className="mt-2 text-2xl font-bold tracking-tight text-slate-50">{title}</h1><p className="mt-2 max-w-3xl text-sm leading-6 text-slate-500">{description}</p></div>{action}</div>;
}

const tones = {
  cyan: "bg-cyan-400/10 text-cyan-300",
  emerald: "bg-emerald-400/10 text-emerald-300",
  blue: "bg-blue-400/10 text-blue-300",
  violet: "bg-violet-400/10 text-violet-300",
};

function Summary({ icon: Icon, label, value, detail, color }: { icon: typeof Cable; label: string; value: string | number; detail: string; color: keyof typeof tones }) {
  return <Card className="p-4"><div className="flex items-start justify-between"><div><div className="text-[11px] font-medium uppercase tracking-[0.12em] text-slate-500">{label}</div><div className="mt-3 text-2xl font-bold text-slate-100">{value}</div></div><span className={`grid h-9 w-9 place-items-center rounded-lg ${tones[color]}`}><Icon size={18} /></span></div><div className="mt-3 text-[11px] text-slate-600">{detail}</div></Card>;
}

function Skeleton() {
  return <Card className="h-64 animate-pulse p-5"><div className="h-11 w-11 rounded-xl bg-white/[0.04]" /><div className="mt-6 grid grid-cols-2 gap-2">{[1,2,3,4].map((i) => <div key={i} className="h-14 rounded-lg bg-white/[0.025]" />)}</div></Card>;
}

export { PageHeading };

