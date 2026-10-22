import { Cable, RefreshCw } from "lucide-react";
import { AdapterCard } from "../components/adapter-card";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { useRuntimeStore } from "../stores/app-store";
import { PageHeading } from "./dashboard";

export function Adapters({ onRefresh }: { onRefresh: () => void }) {
  const { snapshot, loading } = useRuntimeStore();
  const adapters = snapshot?.adapters ?? [];
  return <section className="page-enter space-y-6">
    <PageHeading eyebrow="Interface inventory" title="Network adapters" description="Physical, wireless, virtual, VPN, tunnel, and loopback interfaces returned by the operating system." action={<Button onClick={onRefresh} disabled={loading}><RefreshCw size={15} className={loading ? "animate-spin" : ""} /> Refresh</Button>} />
    <div className="grid gap-4 xl:grid-cols-2">{adapters.map((adapter) => <AdapterCard key={adapter.id} adapter={adapter} />)}</div>
    {!loading && adapters.length === 0 && <Card className="grid min-h-52 place-items-center p-8 text-center"><div><Cable className="mx-auto text-slate-600" /><p className="mt-3 text-sm text-slate-400">No adapter records are available.</p></div></Card>}
  </section>;
}

