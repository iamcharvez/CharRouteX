import { FileClock, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { listActivityEvents, type ActivityEvent } from "../services/profiles";
import { PageHeading } from "./dashboard";

export function ActivityLogsPage() {
  const [events, setEvents] = useState<ActivityEvent[]>([]); const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(async () => { try { setEvents(await listActivityEvents()); setError(null); } catch (reason) { setError(String(reason)); } }, []);
  useEffect(() => { void refresh(); }, [refresh]);
  return <section className="page-enter space-y-6"><PageHeading eyebrow="Persistent audit trail" title="Activity logs" description="Application and configuration events stored locally in SQLite. Sensitive values are excluded from summary messages." action={<Button onClick={() => void refresh()}><RefreshCw size={14} /> Refresh</Button>} />{error && <p className="text-sm text-rose-300">{error}</p>}<Card className="overflow-hidden"><div className="divide-y divide-white/[0.05]">{events.map((event) => <div key={event.id} className="flex items-start gap-4 px-5 py-4"><span className="mt-0.5 grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-blue-400/10 text-blue-300"><FileClock size={15} /></span><div className="min-w-0 flex-1"><div className="flex items-center gap-2"><span className="text-xs font-semibold text-slate-300">{event.message}</span><span className="rounded bg-white/[0.04] px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-slate-600">{event.category}</span></div><p className="mt-1 text-[11px] text-slate-600">{new Date(event.occurredAt).toLocaleString()} · {event.severity}</p></div></div>)}{events.length === 0 && <div className="grid min-h-48 place-items-center text-sm text-slate-600">No activity has been recorded yet.</div>}</div></Card></section>;
}

